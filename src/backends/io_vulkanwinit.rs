use std::default::Default;
use std::error::Error;
use std::io::Cursor;
use std::mem;
use std::mem::{align_of, size_of, size_of_val}; // TODO: Remove when bumping MSRV to 1.80
use std::os::raw::c_void;

use ash::qcom::rotated_copy_commands;
use ash::util::*;
use ash::vk;
use mlua::IntoLua;
use winit::event_loop;

use std::{
    borrow::Cow, cell::RefCell, ffi, ops::Drop, os::raw::c_char,
};

use ash::{
    ext::debug_utils,
    khr::{surface, swapchain},
    Device, Entry, Instance,
};

use winit::{
    application::ApplicationHandler,
    event::{ElementState, Event, KeyEvent, WindowEvent},
    event_loop::{ControlFlow, EventLoop, ActiveEventLoop},
    keyboard::{Key, NamedKey},
    platform::run_on_demand::EventLoopExtRunOnDemand,
    raw_window_handle::{HasDisplayHandle, HasWindowHandle},
    window::{Window, WindowId, WindowAttributes},
};

use crate::api::io::IOBackend;

pub const MAX_FRAME_LATENCY: usize = 1;

#[derive(Clone, Debug, Copy)]
struct Vertex {
    pos: [f32; 4],
    uv: [f32; 2],
}

#[derive(Clone, Debug, Copy)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Clone, Debug, Copy)]
pub struct Mat4 {
    pub data: [[f32; 4]; 4],
}

pub fn mat4_mul(a: &Mat4, b: &Mat4) -> Mat4 {
    let mut result = Mat4 {
        data: [[0.0; 4]; 4],
    };
    for i in 0..4 {
        for j in 0..4 {
            for k in 0..4 {
                result.data[i][j] += a.data[i][k] * b.data[k][j];
            }
        }
    }
    result
}

pub struct VulkanBackend {
    pub entry: Entry,
    pub instance: Instance,
    pub device: Device,
    pub surface_loader: surface::Instance,
    pub swapchain_loader: swapchain::Device,
    pub debug_utils_loader: debug_utils::Instance,
    pub window: winit::window::Window,
    pub frame_index: RefCell<usize>,
    pub debug_call_back: vk::DebugUtilsMessengerEXT,

    pub pdevice: vk::PhysicalDevice,
    pub device_memory_properties: vk::PhysicalDeviceMemoryProperties,
    pub queue_family_index: u32,
    pub present_queue: vk::Queue,

    pub surface: vk::SurfaceKHR,
    pub surface_format: vk::SurfaceFormatKHR,
    pub surface_resolution: vk::Extent2D,

    pub swapchain: vk::SwapchainKHR,
    pub present_images: Vec<vk::Image>,
    pub present_image_views: Vec<vk::ImageView>,

    pub pool: vk::CommandPool,
    pub draw_command_buffers: [vk::CommandBuffer; MAX_FRAME_LATENCY],
    pub setup_command_buffer: vk::CommandBuffer,
    pub app_setup_command_buffer: vk::CommandBuffer,

    pub depth_image: vk::Image,
    pub depth_image_view: vk::ImageView,
    pub depth_image_memory: vk::DeviceMemory,

    pub present_complete_semaphores: [vk::Semaphore; MAX_FRAME_LATENCY],
    pub rendering_complete_semaphores: Vec<vk::Semaphore>,

    pub draw_commands_reuse_fences: [vk::Fence; MAX_FRAME_LATENCY],
    pub renderpass: vk::RenderPass,

    pub framebuffers: Vec<vk::Framebuffer>,
    pub pipeline_layout: vk::PipelineLayout,
    pub descriptor_sets: Vec<vk::DescriptorSet>,
    pub graphic_pipeline: vk::Pipeline,
    pub vertex_input_buffer: vk::Buffer,
    pub index_buffer: vk::Buffer,
    pub index_buffer_data: [u32; 6],
    pub viewports: [vk::Viewport; MAX_FRAME_LATENCY],
    pub scissors: [vk::Rect2D; MAX_FRAME_LATENCY],

    pub uniform_ptr: *mut c_void,
    pub uniform_color_buffer_memory_req: vk::MemoryRequirements,
    pub i: f32,
}

pub fn boxed() -> Box<dyn IOBackend> {
    Box::new(VKWinitBackend::new())
}

#[macro_export]
macro_rules! offset_of {
    ($base:path, $field:ident) => {{
        #[allow(unused_unsafe)]
        unsafe {
            let b: $base = mem::zeroed();
            std::ptr::addr_of!(b.$field) as isize - std::ptr::addr_of!(b) as isize
        }
    }};
}

#[allow(clippy::too_many_arguments)]
pub fn record_submit_commandbuffer<F: FnOnce(&Device, vk::CommandBuffer)>(
    device: &Device,
    command_buffer: vk::CommandBuffer,
    command_buffer_reuse_fence: vk::Fence,
    submit_queue: vk::Queue,
    wait_mask: &[vk::PipelineStageFlags],
    wait_semaphores: &[vk::Semaphore],
    signal_semaphores: &[vk::Semaphore],
    f: F,
) {
    unsafe {
        device
            .reset_command_buffer(
                command_buffer,
                vk::CommandBufferResetFlags::RELEASE_RESOURCES,
            )
            .expect("Reset command buffer failed.");

        let command_buffer_begin_info = vk::CommandBufferBeginInfo::default()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

        device
            .begin_command_buffer(command_buffer, &command_buffer_begin_info)
            .expect("Begin commandbuffer");
        f(device, command_buffer);
        device
            .end_command_buffer(command_buffer)
            .expect("End commandbuffer");

        let command_buffers = vec![command_buffer];

        let submit_info = vk::SubmitInfo::default()
            .wait_semaphores(wait_semaphores)
            .wait_dst_stage_mask(wait_mask)
            .command_buffers(&command_buffers)
            .signal_semaphores(signal_semaphores);

        device
            .queue_submit(submit_queue, &[submit_info], command_buffer_reuse_fence)
            .expect("queue submit failed.");
    }
}

unsafe extern "system" fn vulkan_debug_callback(
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT<'_>,
    _user_data: *mut std::os::raw::c_void,
) -> vk::Bool32 {
    let callback_data = *p_callback_data;
    let message_id_number = callback_data.message_id_number;

    let message_id_name = if callback_data.p_message_id_name.is_null() {
        Cow::from("")
    } else {
        ffi::CStr::from_ptr(callback_data.p_message_id_name).to_string_lossy()
    };

    let message = if callback_data.p_message.is_null() {
        Cow::from("")
    } else {
        ffi::CStr::from_ptr(callback_data.p_message).to_string_lossy()
    };

    println!(
        "{message_severity:?}:\n{message_type:?} [{message_id_name} ({message_id_number})] : {message}\n",
    );

    vk::FALSE
}

pub fn find_memorytype_index(
    memory_req: &vk::MemoryRequirements,
    memory_prop: &vk::PhysicalDeviceMemoryProperties,
    flags: vk::MemoryPropertyFlags,
) -> Option<u32> {
    memory_prop.memory_types[..memory_prop.memory_type_count as _]
        .iter()
        .enumerate()
        .find(|(index, memory_type)| {
            (1 << index) & memory_req.memory_type_bits != 0
                && memory_type.property_flags & flags == flags
        })
        .map(|(index, _memory_type)| index as _)
}

struct VKWinitBackend {
    vulkan: Option<VulkanBackend>,
    started: std::time::Instant,
}

impl IOBackend for VKWinitBackend {
    fn name(&self) -> &str {
        "vkwinit"
    }

    fn init(&mut self) {
        // Initialization logic can be added here if needed
        let event_loop = EventLoop::new().unwrap();

        event_loop.set_control_flow(ControlFlow::Poll);

        event_loop.run_app(self);
    }
}

impl VKWinitBackend {
    pub fn new() -> Self {
        Self { vulkan: None, started: std::time::Instant::now()}
    }
}

impl ApplicationHandler for VKWinitBackend {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.vulkan = Some(VulkanBackend::new(1920, 1080, event_loop).unwrap());
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let v = self.vulkan.as_mut().unwrap();
        unsafe {
            let idx = self.started.elapsed().as_millis() as f32 / 200.0;
            let uniform_color_buffer_data = world_matrix(Vec3 {
                    x: 3.0*idx.sin(),
                    y: 0.5*idx.cos(),
                    z: 3.0*idx.cos(),
                }, Vec3 {
                    x: 0.0,
                    y: -idx,
                    z: 0.0,
                });
                
            let mut uniform_aligned_slice = Align::new(
                v.uniform_ptr,
                align_of::<Mat4>() as u64,
                v.uniform_color_buffer_memory_req.size,
            );
            uniform_aligned_slice.copy_from_slice(&[uniform_color_buffer_data]);
        }
        self.vulkan.as_ref().unwrap().window.request_redraw();
    }

    fn window_event(
            &mut self,
            event_loop: &ActiveEventLoop,
            window_id: winit::window::WindowId,
            event: WindowEvent,
        ) {
            unsafe {
            match event {
                WindowEvent::CloseRequested => { event_loop.exit(); }
                WindowEvent::RedrawRequested => {
                    let v = self.vulkan.as_mut().unwrap();
                    let mut frame_index = v.frame_index.borrow_mut();

                    // The fence from 3 frames ago, that will also be signaled this frame
                    let draw_commands_reuse_fence =
                        v.draw_commands_reuse_fences[*frame_index % MAX_FRAME_LATENCY];
                    unsafe {
                        v.device.wait_for_fences(&[draw_commands_reuse_fence], true, u64::MAX)
                    }
                    .expect("Wait for fence failed.");

                    unsafe { v.device.reset_fences(&[draw_commands_reuse_fence]) }
                        .expect("Reset fences failed.");

                    let f = |frame_index| {
                        let present_complete_semaphore =
                            v.present_complete_semaphores[frame_index % MAX_FRAME_LATENCY];
                        let draw_commands_reuse_fence =
                        v.draw_commands_reuse_fences[frame_index % MAX_FRAME_LATENCY];
                        let draw_command_buffer = v.draw_command_buffers[frame_index % MAX_FRAME_LATENCY];
        
                        let (present_index, _) = v.swapchain_loader
                            .acquire_next_image(
                                v.swapchain,
                                u64::MAX,
                                present_complete_semaphore,
                                vk::Fence::null(),
                            )
                            .unwrap();
                        let clear_values = [
                            vk::ClearValue {
                                color: vk::ClearColorValue {
                                    float32: [0.1, 0.1, 0.1, 0.0],
                                },
                            },
                            vk::ClearValue {
                                depth_stencil: vk::ClearDepthStencilValue {
                                    depth: 1.0,
                                    stencil: 0,
                                },
                            },
                        ];
        
                        let rendering_complete_semaphore =
                            v.rendering_complete_semaphores[present_index as usize];
        
                        let render_pass_begin_info = vk::RenderPassBeginInfo::default()
                            .render_pass(v.renderpass)
                            .framebuffer(v.framebuffers[present_index as usize])
                            .render_area(v.surface_resolution.into())
                            .clear_values(&clear_values);
        
                        record_submit_commandbuffer(
                            &v.device,
                            draw_command_buffer,
                            draw_commands_reuse_fence,
                            v.present_queue,
                            &[vk::PipelineStageFlags::BOTTOM_OF_PIPE],
                            &[present_complete_semaphore],
                            &[rendering_complete_semaphore],
                            |device, draw_command_buffer| {
                                device.cmd_begin_render_pass(
                                    draw_command_buffer,
                                    &render_pass_begin_info,
                                    vk::SubpassContents::INLINE,
                                );
                                device.cmd_bind_descriptor_sets(
                                    draw_command_buffer,
                                    vk::PipelineBindPoint::GRAPHICS,
                                    v.pipeline_layout,
                                    0,
                                    &v.descriptor_sets[..],
                                    &[],
                                );
                                device.cmd_bind_pipeline(
                                    draw_command_buffer,
                                    vk::PipelineBindPoint::GRAPHICS,
                                    v.graphic_pipeline,
                                );
                                device.cmd_set_viewport(draw_command_buffer, 0, &v.viewports);
                                device.cmd_set_scissor(draw_command_buffer, 0, &v.scissors);
                                device.cmd_bind_vertex_buffers(
                                    draw_command_buffer,
                                    0,
                                    &[v.vertex_input_buffer],
                                    &[0],
                                );
                                device.cmd_bind_index_buffer(
                                    draw_command_buffer,
                                    v.index_buffer,
                                    0,
                                    vk::IndexType::UINT32,
                                );
                                device.cmd_draw_indexed(
                                    draw_command_buffer,
                                    v.index_buffer_data.len() as u32,
                                    1,
                                    0,
                                    0,
                                    1,
                                );
                                // Or draw without the index buffer
                                // device.cmd_draw(draw_command_buffer, 3, 1, 0, 0);
                                device.cmd_end_render_pass(draw_command_buffer);
                            },
                        );
                        let present_info = vk::PresentInfoKHR {
                            wait_semaphore_count: 1,
                            p_wait_semaphores: &rendering_complete_semaphore,
                            swapchain_count: 1,
                            p_swapchains: &v.swapchain,
                            p_image_indices: &present_index,
                            ..Default::default()
                        };
                        v.swapchain_loader
                            .queue_present(v.present_queue, &present_info)
                            .unwrap();
                    };

                    f(*frame_index);
                    *frame_index += 1;
                }
                _ => (),
            }
        }
    }
}

impl VulkanBackend {
    pub fn new(window_width: u32, window_height: u32, event_loop: &ActiveEventLoop) -> Result<Self, Box<dyn Error>> {
        unsafe {
            let attributes: WindowAttributes = Window::default_attributes()
                .with_title("Vulkan Triangle")
                .with_inner_size(winit::dpi::PhysicalSize::new(window_width, window_height));
            let window = event_loop.create_window(attributes)?;
            let entry = Entry::linked();
            let app_name = c"VulkanTriangle";

            let layer_names = [c"VK_LAYER_KHRONOS_validation"];
            let layers_names_raw: Vec<*const c_char> = layer_names
                .iter()
                .map(|raw_name| raw_name.as_ptr())
                .collect();

            let mut extension_names =
                ash_window::enumerate_required_extensions(event_loop.display_handle()?.as_raw())
                    .unwrap()
                    .to_vec();
            extension_names.push(debug_utils::NAME.as_ptr());

            #[cfg(any(target_os = "macos", target_os = "ios"))]
            {
                extension_names.push(ash::khr::portability_enumeration::NAME.as_ptr());
                // Enabling this extension is a requirement when using `VK_KHR_portability_subset`
                extension_names.push(ash::khr::get_physical_device_properties2::NAME.as_ptr());
            }

            let appinfo = vk::ApplicationInfo::default()
                .application_name(app_name)
                .application_version(0)
                .engine_name(app_name)
                .engine_version(0)
                .api_version(vk::make_api_version(0, 1, 0, 0));

            let create_flags = if cfg!(any(target_os = "macos", target_os = "ios")) {
                vk::InstanceCreateFlags::ENUMERATE_PORTABILITY_KHR
            } else {
                vk::InstanceCreateFlags::default()
            };

            let create_info = vk::InstanceCreateInfo::default()
                .application_info(&appinfo)
                .enabled_layer_names(&layers_names_raw)
                .enabled_extension_names(&extension_names)
                .flags(create_flags);

            let instance: Instance = entry
                .create_instance(&create_info, None)
                .expect("Instance creation error");

            let debug_info = vk::DebugUtilsMessengerCreateInfoEXT::default()
                .message_severity(
                    vk::DebugUtilsMessageSeverityFlagsEXT::ERROR
                        | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                        | vk::DebugUtilsMessageSeverityFlagsEXT::INFO,
                )
                .message_type(
                    vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                        | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                        | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
                )
                .pfn_user_callback(Some(vulkan_debug_callback));

            let debug_utils_loader = debug_utils::Instance::new(&entry, &instance);
            let debug_call_back = debug_utils_loader
                .create_debug_utils_messenger(&debug_info, None)
                .unwrap();

            let surface = ash_window::create_surface(&entry, &instance, event_loop.display_handle()?.as_raw(), window.window_handle()?.as_raw(), None)?;
            let surface_loader = surface::Instance::new(&entry, &instance);

            let pdevices = instance
                .enumerate_physical_devices()
                .expect("Physical device error");
            let (pdevice, queue_family_index) = pdevices
                .iter()
                .find_map(|pdevice| {
                    instance
                        .get_physical_device_queue_family_properties(*pdevice)
                        .iter()
                        .enumerate()
                        .find_map(|(index, info)| {
                            let supports_graphic_and_surface =
                                info.queue_flags.contains(vk::QueueFlags::GRAPHICS)
                                    && surface_loader
                                        .get_physical_device_surface_support(
                                            *pdevice,
                                            index as u32,
                                            surface,
                                        )
                                        .unwrap();
                            if supports_graphic_and_surface {
                                Some((*pdevice, index))
                            } else {
                                None
                            }
                        })
                })
                .expect("Couldn't find suitable device.");
            let queue_family_index = queue_family_index as u32;
            let device_extension_names_raw = [
                swapchain::NAME.as_ptr(),
                #[cfg(any(target_os = "macos", target_os = "ios"))]
                ash::khr::portability_subset::NAME.as_ptr(),
            ];
            let features = vk::PhysicalDeviceFeatures {
                shader_clip_distance: 1,
                ..Default::default()
            };
            let priorities = [1.0];

            let queue_info = vk::DeviceQueueCreateInfo::default()
                .queue_family_index(queue_family_index)
                .queue_priorities(&priorities);

            let device_create_info = vk::DeviceCreateInfo::default()
                .queue_create_infos(std::slice::from_ref(&queue_info))
                .enabled_extension_names(&device_extension_names_raw)
                .enabled_features(&features);

            let device: Device = instance
                .create_device(pdevice, &device_create_info, None)
                .unwrap();

            let present_queue = device.get_device_queue(queue_family_index, 0);

            let surface_format = surface_loader
                .get_physical_device_surface_formats(pdevice, surface)
                .unwrap()[0];

            let surface_capabilities = surface_loader
                .get_physical_device_surface_capabilities(pdevice, surface)
                .unwrap();
            let mut desired_image_count = surface_capabilities.min_image_count + 1;
            if surface_capabilities.max_image_count > 0
                && desired_image_count > surface_capabilities.max_image_count
            {
                desired_image_count = surface_capabilities.max_image_count;
            }
            let surface_resolution = match surface_capabilities.current_extent.width {
                u32::MAX => vk::Extent2D {
                    width: window_width,
                    height: window_height,
                },
                _ => surface_capabilities.current_extent,
            };
            let pre_transform = if surface_capabilities
                .supported_transforms
                .contains(vk::SurfaceTransformFlagsKHR::IDENTITY)
            {
                vk::SurfaceTransformFlagsKHR::IDENTITY
            } else {
                surface_capabilities.current_transform
            };
            let present_modes = surface_loader
                .get_physical_device_surface_present_modes(pdevice, surface)
                .unwrap();
            let present_mode = present_modes
                .iter()
                .cloned()
                .find(|&mode| mode == vk::PresentModeKHR::MAILBOX)
                .unwrap_or(vk::PresentModeKHR::FIFO);
            let swapchain_loader = swapchain::Device::new(&instance, &device);

            let swapchain_create_info = vk::SwapchainCreateInfoKHR::default()
                .surface(surface)
                .min_image_count(desired_image_count)
                .image_color_space(surface_format.color_space)
                .image_format(surface_format.format)
                .image_extent(surface_resolution)
                .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
                .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
                .pre_transform(pre_transform)
                .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
                .present_mode(present_mode)
                .clipped(true)
                .image_array_layers(1);

            let swapchain = swapchain_loader
                .create_swapchain(&swapchain_create_info, None)
                .unwrap();

            let pool_create_info = vk::CommandPoolCreateInfo::default()
                .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
                .queue_family_index(queue_family_index);

            let pool = device.create_command_pool(&pool_create_info, None).unwrap();

            let command_buffer_allocate_info = vk::CommandBufferAllocateInfo::default()
                .command_buffer_count(2 + MAX_FRAME_LATENCY as u32)
                .command_pool(pool)
                .level(vk::CommandBufferLevel::PRIMARY);

            let command_buffers = device
                .allocate_command_buffers(&command_buffer_allocate_info)
                .unwrap();
            let setup_command_buffer = command_buffers[0];
            let app_setup_command_buffer = command_buffers[1];
            let draw_command_buffers = command_buffers[2..][..MAX_FRAME_LATENCY]
                .try_into()
                .unwrap();

            let present_images = swapchain_loader.get_swapchain_images(swapchain).unwrap();
            let present_image_views: Vec<vk::ImageView> = present_images
                .iter()
                .map(|&image| {
                    let create_view_info = vk::ImageViewCreateInfo::default()
                        .view_type(vk::ImageViewType::TYPE_2D)
                        .format(surface_format.format)
                        .components(vk::ComponentMapping {
                            r: vk::ComponentSwizzle::R,
                            g: vk::ComponentSwizzle::G,
                            b: vk::ComponentSwizzle::B,
                            a: vk::ComponentSwizzle::A,
                        })
                        .subresource_range(vk::ImageSubresourceRange {
                            aspect_mask: vk::ImageAspectFlags::COLOR,
                            base_mip_level: 0,
                            level_count: 1,
                            base_array_layer: 0,
                            layer_count: 1,
                        })
                        .image(image);
                    device.create_image_view(&create_view_info, None).unwrap()
                })
                .collect();
            let device_memory_properties = instance.get_physical_device_memory_properties(pdevice);
            let depth_image_create_info = vk::ImageCreateInfo::default()
                .image_type(vk::ImageType::TYPE_2D)
                .format(vk::Format::D16_UNORM)
                .extent(surface_resolution.into())
                .mip_levels(1)
                .array_layers(1)
                .samples(vk::SampleCountFlags::TYPE_1)
                .tiling(vk::ImageTiling::OPTIMAL)
                .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT)
                .sharing_mode(vk::SharingMode::EXCLUSIVE);

            let depth_image = device.create_image(&depth_image_create_info, None).unwrap();
            let depth_image_memory_req = device.get_image_memory_requirements(depth_image);
            let depth_image_memory_index = find_memorytype_index(
                &depth_image_memory_req,
                &device_memory_properties,
                vk::MemoryPropertyFlags::DEVICE_LOCAL,
            )
            .expect("Unable to find suitable memory index for depth image.");

            let depth_image_allocate_info = vk::MemoryAllocateInfo::default()
                .allocation_size(depth_image_memory_req.size)
                .memory_type_index(depth_image_memory_index);

            let depth_image_memory = device
                .allocate_memory(&depth_image_allocate_info, None)
                .unwrap();

            device
                .bind_image_memory(depth_image, depth_image_memory, 0)
                .expect("Unable to bind depth image memory");

            record_submit_commandbuffer(
                &device,
                setup_command_buffer,
                vk::Fence::null(),
                present_queue,
                &[],
                &[],
                &[],
                |device, setup_command_buffer| {
                    let layout_transition_barriers = vk::ImageMemoryBarrier::default()
                        .image(depth_image)
                        .dst_access_mask(
                            vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ
                                | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                        )
                        .new_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
                        .old_layout(vk::ImageLayout::UNDEFINED)
                        .subresource_range(
                            vk::ImageSubresourceRange::default()
                                .aspect_mask(vk::ImageAspectFlags::DEPTH)
                                .layer_count(1)
                                .level_count(1),
                        );

                    device.cmd_pipeline_barrier(
                        setup_command_buffer,
                        vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                        vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
                        vk::DependencyFlags::empty(),
                        &[],
                        &[],
                        &[layout_transition_barriers],
                    );
                },
            );

            let depth_image_view_info = vk::ImageViewCreateInfo::default()
                .subresource_range(
                    vk::ImageSubresourceRange::default()
                        .aspect_mask(vk::ImageAspectFlags::DEPTH)
                        .level_count(1)
                        .layer_count(1),
                )
                .image(depth_image)
                .format(depth_image_create_info.format)
                .view_type(vk::ImageViewType::TYPE_2D);

            let depth_image_view = device
                .create_image_view(&depth_image_view_info, None)
                .unwrap();

            let semaphore_create_info = vk::SemaphoreCreateInfo::default();

            let present_complete_semaphores = std::array::from_fn(|_| {
                device
                    .create_semaphore(&semaphore_create_info, None)
                    .unwrap()
            });
            let rendering_complete_semaphores = (0..present_images.len())
                .map(|_| {
                    device
                        .create_semaphore(&semaphore_create_info, None)
                        .unwrap()
                })
                .collect();

            let fence_create_info =
                vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED);

            let draw_commands_reuse_fences = std::array::from_fn(|_| {
                device
                    .create_fence(&fence_create_info, None)
                    .expect("Create fence failed.")
            });

            let renderpass_attachments = [
            vk::AttachmentDescription {
                format: surface_format.format,
                samples: vk::SampleCountFlags::TYPE_1,
                load_op: vk::AttachmentLoadOp::CLEAR,
                store_op: vk::AttachmentStoreOp::STORE,
                final_layout: vk::ImageLayout::PRESENT_SRC_KHR,
                ..Default::default()
            },
            vk::AttachmentDescription {
                format: vk::Format::D16_UNORM,
                samples: vk::SampleCountFlags::TYPE_1,
                load_op: vk::AttachmentLoadOp::CLEAR,
                initial_layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                final_layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                ..Default::default()
            },
        ];
        let color_attachment_refs = [vk::AttachmentReference {
            attachment: 0,
            layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
        }];
        let depth_attachment_ref = vk::AttachmentReference {
            attachment: 1,
            layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
        };
        let dependencies = [vk::SubpassDependency {
            src_subpass: vk::SUBPASS_EXTERNAL,
            src_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            dst_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_READ
                | vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
            dst_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            ..Default::default()
        }];

        let subpass = vk::SubpassDescription::default()
            .color_attachments(&color_attachment_refs)
            .depth_stencil_attachment(&depth_attachment_ref)
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS);

        let renderpass_create_info = vk::RenderPassCreateInfo::default()
            .attachments(&renderpass_attachments)
            .subpasses(std::slice::from_ref(&subpass))
            .dependencies(&dependencies);

        let renderpass = device
            .create_render_pass(&renderpass_create_info, None)
            .unwrap();

        let framebuffers: Vec<vk::Framebuffer> = present_image_views
            .iter()
            .map(|&present_image_view| {
                let framebuffer_attachments = [present_image_view, depth_image_view];
                let frame_buffer_create_info = vk::FramebufferCreateInfo::default()
                    .render_pass(renderpass)
                    .attachments(&framebuffer_attachments)
                    .width(surface_resolution.width)
                    .height(surface_resolution.height)
                    .layers(1);

                device
                    .create_framebuffer(&frame_buffer_create_info, None)
                    .unwrap()
            })
            .collect();
        let index_buffer_data = [0u32, 1, 2, 2, 3, 0];
        let index_buffer_info = vk::BufferCreateInfo {
            size: size_of_val(&index_buffer_data) as u64,
            usage: vk::BufferUsageFlags::INDEX_BUFFER,
            sharing_mode: vk::SharingMode::EXCLUSIVE,
            ..Default::default()
        };
        let index_buffer = device.create_buffer(&index_buffer_info, None).unwrap();
        let index_buffer_memory_req = device.get_buffer_memory_requirements(index_buffer);
        let index_buffer_memory_index = find_memorytype_index(
            &index_buffer_memory_req,
            &device_memory_properties,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        )
        .expect("Unable to find suitable memorytype for the index buffer.");
        let index_allocate_info = vk::MemoryAllocateInfo {
            allocation_size: index_buffer_memory_req.size,
            memory_type_index: index_buffer_memory_index,
            ..Default::default()
        };
        let index_buffer_memory = device
            .allocate_memory(&index_allocate_info, None)
            .unwrap();
        let index_ptr: *mut c_void = device
            .map_memory(
                index_buffer_memory,
                0,
                index_buffer_memory_req.size,
                vk::MemoryMapFlags::empty(),
            )
            .unwrap();
        let mut index_slice = Align::new(
            index_ptr,
            align_of::<u32>() as u64,
            index_buffer_memory_req.size,
        );
        index_slice.copy_from_slice(&index_buffer_data);
        device.unmap_memory(index_buffer_memory);
        device
            .bind_buffer_memory(index_buffer, index_buffer_memory, 0)
            .unwrap();

        let vertices = [
            Vertex {
                pos: [-1.0, -1.0, 0.0, 1.0],
                uv: [0.0, 0.0],
            },
            Vertex {
                pos: [-1.0, 1.0, 0.0, 1.0],
                uv: [0.0, 1.0],
            },
            Vertex {
                pos: [1.0, 1.0, 0.0, 1.0],
                uv: [1.0, 1.0],
            },
            Vertex {
                pos: [1.0, -1.0, 0.0, 1.0],
                uv: [1.0, 0.0],
            },
        ];
        let vertex_input_buffer_info = vk::BufferCreateInfo {
            size: size_of_val(&vertices) as u64,
            usage: vk::BufferUsageFlags::VERTEX_BUFFER,
            sharing_mode: vk::SharingMode::EXCLUSIVE,
            ..Default::default()
        };
        let vertex_input_buffer = device
            .create_buffer(&vertex_input_buffer_info, None)
            .unwrap();
        let vertex_input_buffer_memory_req = device
            .get_buffer_memory_requirements(vertex_input_buffer);
        let vertex_input_buffer_memory_index = find_memorytype_index(
            &vertex_input_buffer_memory_req,
            &device_memory_properties,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        )
        .expect("Unable to find suitable memorytype for the vertex buffer.");

        let vertex_buffer_allocate_info = vk::MemoryAllocateInfo {
            allocation_size: vertex_input_buffer_memory_req.size,
            memory_type_index: vertex_input_buffer_memory_index,
            ..Default::default()
        };
        let vertex_input_buffer_memory = device
            .allocate_memory(&vertex_buffer_allocate_info, None)
            .unwrap();

        let vert_ptr = device
            .map_memory(
                vertex_input_buffer_memory,
                0,
                vertex_input_buffer_memory_req.size,
                vk::MemoryMapFlags::empty(),
            )
            .unwrap();
        let mut slice = Align::new(
            vert_ptr,
            align_of::<Vertex>() as u64,
            vertex_input_buffer_memory_req.size,
        );
        slice.copy_from_slice(&vertices);
        device.unmap_memory(vertex_input_buffer_memory);
        device
            .bind_buffer_memory(vertex_input_buffer, vertex_input_buffer_memory, 0)
            .unwrap();

        let uniform_color_buffer_data = Mat4 {
            data: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ]
        };
        let uniform_color_buffer_info = vk::BufferCreateInfo {
            size: size_of_val(&uniform_color_buffer_data) as u64,
            usage: vk::BufferUsageFlags::UNIFORM_BUFFER,
            sharing_mode: vk::SharingMode::EXCLUSIVE,
            ..Default::default()
        };
        let uniform_color_buffer = device
            .create_buffer(&uniform_color_buffer_info, None)
            .unwrap();
        let uniform_color_buffer_memory_req = device
            .get_buffer_memory_requirements(uniform_color_buffer);
        let uniform_color_buffer_memory_index = find_memorytype_index(
            &uniform_color_buffer_memory_req,
            &device_memory_properties,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        )
        .expect("Unable to find suitable memorytype for the vertex buffer.");

        let uniform_color_buffer_allocate_info = vk::MemoryAllocateInfo {
            allocation_size: uniform_color_buffer_memory_req.size,
            memory_type_index: uniform_color_buffer_memory_index,
            ..Default::default()
        };
        let uniform_color_buffer_memory = device
            .allocate_memory(&uniform_color_buffer_allocate_info, None)
            .unwrap();
        let uniform_ptr = device
            .map_memory(
                uniform_color_buffer_memory,
                0,
                uniform_color_buffer_memory_req.size,
                vk::MemoryMapFlags::empty(),
            )
            .unwrap();
        let mut uniform_aligned_slice = Align::new(
            uniform_ptr,
            align_of::<Mat4>() as u64,
            uniform_color_buffer_memory_req.size,
        );
        uniform_aligned_slice.copy_from_slice(&[uniform_color_buffer_data]);
        //device.unmap_memory(uniform_color_buffer_memory);
        device
            .bind_buffer_memory(uniform_color_buffer, uniform_color_buffer_memory, 0)
            .unwrap();

        let image = image::load_from_memory(include_bytes!("../../assets/image.png"))
            .unwrap()
            .to_rgba8();
        let (width, height) = image.dimensions();
        let image_extent = vk::Extent2D { width, height };
        let image_data = image.into_raw();
        let image_buffer_info = vk::BufferCreateInfo {
            size: (size_of::<u8>() * image_data.len()) as u64,
            usage: vk::BufferUsageFlags::TRANSFER_SRC,
            sharing_mode: vk::SharingMode::EXCLUSIVE,
            ..Default::default()
        };
        let image_buffer = device.create_buffer(&image_buffer_info, None).unwrap();
        let image_buffer_memory_req = device.get_buffer_memory_requirements(image_buffer);
        let image_buffer_memory_index = find_memorytype_index(
            &image_buffer_memory_req,
            &device_memory_properties,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        )
        .expect("Unable to find suitable memorytype for the image buffer.");

        let image_buffer_allocate_info = vk::MemoryAllocateInfo {
            allocation_size: image_buffer_memory_req.size,
            memory_type_index: image_buffer_memory_index,
            ..Default::default()
        };
        let image_buffer_memory = device
            .allocate_memory(&image_buffer_allocate_info, None)
            .unwrap();
        let image_ptr = device
            .map_memory(
                image_buffer_memory,
                0,
                image_buffer_memory_req.size,
                vk::MemoryMapFlags::empty(),
            )
            .unwrap();
        let mut image_slice = Align::new(
            image_ptr,
            align_of::<u8>() as u64,
            image_buffer_memory_req.size,
        );
        image_slice.copy_from_slice(&image_data);
        device.unmap_memory(image_buffer_memory);
        device
            .bind_buffer_memory(image_buffer, image_buffer_memory, 0)
            .unwrap();

        let texture_create_info = vk::ImageCreateInfo {
            image_type: vk::ImageType::TYPE_2D,
            format: vk::Format::R8G8B8A8_UNORM,
            extent: image_extent.into(),
            mip_levels: 1,
            array_layers: 1,
            samples: vk::SampleCountFlags::TYPE_1,
            tiling: vk::ImageTiling::OPTIMAL,
            usage: vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED,
            sharing_mode: vk::SharingMode::EXCLUSIVE,
            ..Default::default()
        };
        let texture_image = device
            .create_image(&texture_create_info, None)
            .unwrap();
        let texture_memory_req = device.get_image_memory_requirements(texture_image);
        let texture_memory_index = find_memorytype_index(
            &texture_memory_req,
            &device_memory_properties,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
        )
        .expect("Unable to find suitable memory index for depth image.");

        let texture_allocate_info = vk::MemoryAllocateInfo {
            allocation_size: texture_memory_req.size,
            memory_type_index: texture_memory_index,
            ..Default::default()
        };
        let texture_memory = device
            .allocate_memory(&texture_allocate_info, None)
            .unwrap();
        device
            .bind_image_memory(texture_image, texture_memory, 0)
            .expect("Unable to bind depth image memory");

        record_submit_commandbuffer(
            &device,
            app_setup_command_buffer,
            vk::Fence::null(),
            present_queue,
            &[],
            &[],
            &[],
            |device, texture_command_buffer| {
                let texture_barrier = vk::ImageMemoryBarrier {
                    dst_access_mask: vk::AccessFlags::TRANSFER_WRITE,
                    new_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    image: texture_image,
                    subresource_range: vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        level_count: 1,
                        layer_count: 1,
                        ..Default::default()
                    },
                    ..Default::default()
                };
                device.cmd_pipeline_barrier(
                    texture_command_buffer,
                    vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::DependencyFlags::empty(),
                    &[],
                    &[],
                    &[texture_barrier],
                );
                let buffer_copy_regions = vk::BufferImageCopy::default()
                    .image_subresource(
                        vk::ImageSubresourceLayers::default()
                            .aspect_mask(vk::ImageAspectFlags::COLOR)
                            .layer_count(1),
                    )
                    .image_extent(image_extent.into());

                device.cmd_copy_buffer_to_image(
                    texture_command_buffer,
                    image_buffer,
                    texture_image,
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    &[buffer_copy_regions],
                );
                let texture_barrier_end = vk::ImageMemoryBarrier {
                    src_access_mask: vk::AccessFlags::TRANSFER_WRITE,
                    dst_access_mask: vk::AccessFlags::SHADER_READ,
                    old_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    new_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                    image: texture_image,
                    subresource_range: vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        level_count: 1,
                        layer_count: 1,
                        ..Default::default()
                    },
                    ..Default::default()
                };
                device.cmd_pipeline_barrier(
                    texture_command_buffer,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::PipelineStageFlags::FRAGMENT_SHADER,
                    vk::DependencyFlags::empty(),
                    &[],
                    &[],
                    &[texture_barrier_end],
                );
            },
        );

        let sampler_info = vk::SamplerCreateInfo {
            mag_filter: vk::Filter::LINEAR,
            min_filter: vk::Filter::LINEAR,
            mipmap_mode: vk::SamplerMipmapMode::LINEAR,
            address_mode_u: vk::SamplerAddressMode::MIRRORED_REPEAT,
            address_mode_v: vk::SamplerAddressMode::MIRRORED_REPEAT,
            address_mode_w: vk::SamplerAddressMode::MIRRORED_REPEAT,
            max_anisotropy: 1.0,
            border_color: vk::BorderColor::FLOAT_OPAQUE_WHITE,
            compare_op: vk::CompareOp::NEVER,
            ..Default::default()
        };

        let sampler = device.create_sampler(&sampler_info, None).unwrap();

        let tex_image_view_info = vk::ImageViewCreateInfo {
            view_type: vk::ImageViewType::TYPE_2D,
            format: texture_create_info.format,
            components: vk::ComponentMapping {
                r: vk::ComponentSwizzle::R,
                g: vk::ComponentSwizzle::G,
                b: vk::ComponentSwizzle::B,
                a: vk::ComponentSwizzle::A,
            },
            subresource_range: vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                level_count: 1,
                layer_count: 1,
                ..Default::default()
            },
            image: texture_image,
            ..Default::default()
        };
        let tex_image_view = device
            .create_image_view(&tex_image_view_info, None)
            .unwrap();
        let descriptor_sizes = [
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::UNIFORM_BUFFER,
                descriptor_count: 1,
            },
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                descriptor_count: 1,
            },
        ];
        let descriptor_pool_info = vk::DescriptorPoolCreateInfo::default()
            .pool_sizes(&descriptor_sizes)
            .max_sets(1);

        let descriptor_pool = device
            .create_descriptor_pool(&descriptor_pool_info, None)
            .unwrap();
        let desc_layout_bindings = [
            vk::DescriptorSetLayoutBinding {
                descriptor_type: vk::DescriptorType::UNIFORM_BUFFER,
                descriptor_count: 1,
                stage_flags: vk::ShaderStageFlags::VERTEX,
                ..Default::default()
            },
            vk::DescriptorSetLayoutBinding {
                binding: 1,
                descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                descriptor_count: 1,
                stage_flags: vk::ShaderStageFlags::FRAGMENT,
                ..Default::default()
            },
        ];
        let descriptor_info =
            vk::DescriptorSetLayoutCreateInfo::default().bindings(&desc_layout_bindings);

        let desc_set_layouts = [device
            .create_descriptor_set_layout(&descriptor_info, None)
            .unwrap()];

        let desc_alloc_info = vk::DescriptorSetAllocateInfo::default()
            .descriptor_pool(descriptor_pool)
            .set_layouts(&desc_set_layouts);
        let descriptor_sets = device
            .allocate_descriptor_sets(&desc_alloc_info)
            .unwrap();

        let uniform_color_buffer_descriptor = vk::DescriptorBufferInfo {
            buffer: uniform_color_buffer,
            offset: 0,
            range: size_of_val(&uniform_color_buffer_data) as u64,
        };

        let tex_descriptor = vk::DescriptorImageInfo {
            image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            image_view: tex_image_view,
            sampler,
        };

        let write_desc_sets = [
            vk::WriteDescriptorSet {
                dst_set: descriptor_sets[0],
                descriptor_count: 1,
                descriptor_type: vk::DescriptorType::UNIFORM_BUFFER,
                p_buffer_info: &uniform_color_buffer_descriptor,
                ..Default::default()
            },
            vk::WriteDescriptorSet {
                dst_set: descriptor_sets[0],
                dst_binding: 1,
                descriptor_count: 1,
                descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                p_image_info: &tex_descriptor,
                ..Default::default()
            },
        ];
        device.update_descriptor_sets(&write_desc_sets, &[]);

        let mut vertex_spv_file = Cursor::new(&include_bytes!("../../assets/tex_vert.spv")[..]);
        let mut frag_spv_file = Cursor::new(&include_bytes!("../../assets/tex_frag.spv")[..]);

        let vertex_code =
            read_spv(&mut vertex_spv_file).expect("Failed to read vertex shader spv file");
        let vertex_shader_info = vk::ShaderModuleCreateInfo::default().code(&vertex_code);

        let frag_code =
            read_spv(&mut frag_spv_file).expect("Failed to read fragment shader spv file");
        let frag_shader_info = vk::ShaderModuleCreateInfo::default().code(&frag_code);

        let vertex_shader_module = device
            .create_shader_module(&vertex_shader_info, None)
            .expect("Vertex shader module error");

        let fragment_shader_module = device
            .create_shader_module(&frag_shader_info, None)
            .expect("Fragment shader module error");

        let layout_create_info =
            vk::PipelineLayoutCreateInfo::default().set_layouts(&desc_set_layouts);

        let pipeline_layout = device
            .create_pipeline_layout(&layout_create_info, None)
            .unwrap();

        let shader_entry_name = c"main";
        let shader_stage_create_infos = [
            vk::PipelineShaderStageCreateInfo {
                module: vertex_shader_module,
                p_name: shader_entry_name.as_ptr(),
                stage: vk::ShaderStageFlags::VERTEX,
                ..Default::default()
            },
            vk::PipelineShaderStageCreateInfo {
                module: fragment_shader_module,
                p_name: shader_entry_name.as_ptr(),
                stage: vk::ShaderStageFlags::FRAGMENT,
                ..Default::default()
            },
        ];
        let vertex_input_binding_descriptions = [vk::VertexInputBindingDescription {
            binding: 0,
            stride: size_of::<Vertex>() as u32,
            input_rate: vk::VertexInputRate::VERTEX,
        }];
        let vertex_input_attribute_descriptions = [
            vk::VertexInputAttributeDescription {
                location: 0,
                binding: 0,
                format: vk::Format::R32G32B32A32_SFLOAT,
                offset: offset_of!(Vertex, pos) as u32,
            },
            vk::VertexInputAttributeDescription {
                location: 1,
                binding: 0,
                format: vk::Format::R32G32_SFLOAT,
                offset: offset_of!(Vertex, uv) as u32,
            },
        ];
        let vertex_input_state_info = vk::PipelineVertexInputStateCreateInfo::default()
            .vertex_attribute_descriptions(&vertex_input_attribute_descriptions)
            .vertex_binding_descriptions(&vertex_input_binding_descriptions);

        let vertex_input_assembly_state_info = vk::PipelineInputAssemblyStateCreateInfo {
            topology: vk::PrimitiveTopology::TRIANGLE_LIST,
            ..Default::default()
        };
        let viewports = [vk::Viewport {
            x: 0.0,
            y: 0.0,
            width: surface_resolution.width as f32,
            height: surface_resolution.height as f32,
            min_depth: 0.0,
            max_depth: 1.0,
        }];
        let scissors = [surface_resolution.into()];
        let viewport_state_info = vk::PipelineViewportStateCreateInfo::default()
            .scissors(&scissors)
            .viewports(&viewports);

        let rasterization_info = vk::PipelineRasterizationStateCreateInfo {
            front_face: vk::FrontFace::COUNTER_CLOCKWISE,
            line_width: 1.0,
            polygon_mode: vk::PolygonMode::FILL,
            ..Default::default()
        };

        let multisample_state_info = vk::PipelineMultisampleStateCreateInfo::default()
            .rasterization_samples(vk::SampleCountFlags::TYPE_1);

        let noop_stencil_state = vk::StencilOpState {
            fail_op: vk::StencilOp::KEEP,
            pass_op: vk::StencilOp::KEEP,
            depth_fail_op: vk::StencilOp::KEEP,
            compare_op: vk::CompareOp::ALWAYS,
            ..Default::default()
        };
        let depth_state_info = vk::PipelineDepthStencilStateCreateInfo {
            depth_test_enable: 1,
            depth_write_enable: 1,
            depth_compare_op: vk::CompareOp::LESS_OR_EQUAL,
            front: noop_stencil_state,
            back: noop_stencil_state,
            max_depth_bounds: 1.0,
            ..Default::default()
        };

        let color_blend_attachment_states = [vk::PipelineColorBlendAttachmentState {
            blend_enable: 0,
            src_color_blend_factor: vk::BlendFactor::SRC_COLOR,
            dst_color_blend_factor: vk::BlendFactor::ONE_MINUS_DST_COLOR,
            color_blend_op: vk::BlendOp::ADD,
            src_alpha_blend_factor: vk::BlendFactor::ZERO,
            dst_alpha_blend_factor: vk::BlendFactor::ZERO,
            alpha_blend_op: vk::BlendOp::ADD,
            color_write_mask: vk::ColorComponentFlags::RGBA,
        }];
        let color_blend_state = vk::PipelineColorBlendStateCreateInfo::default()
            .logic_op(vk::LogicOp::CLEAR)
            .attachments(&color_blend_attachment_states);

        let dynamic_state = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dynamic_state_info =
            vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_state);

        let graphic_pipeline_infos = vk::GraphicsPipelineCreateInfo::default()
            .stages(&shader_stage_create_infos)
            .vertex_input_state(&vertex_input_state_info)
            .input_assembly_state(&vertex_input_assembly_state_info)
            .viewport_state(&viewport_state_info)
            .rasterization_state(&rasterization_info)
            .multisample_state(&multisample_state_info)
            .depth_stencil_state(&depth_state_info)
            .color_blend_state(&color_blend_state)
            .dynamic_state(&dynamic_state_info)
            .layout(pipeline_layout)
            .render_pass(renderpass);

        let graphics_pipelines = device
            .create_graphics_pipelines(vk::PipelineCache::null(), &[graphic_pipeline_infos], None)
            .unwrap();

        let graphic_pipeline = graphics_pipelines[0];

        let i = 0.0;

        Ok(Self { 
            frame_index: RefCell::new(0),
            entry,
            instance,
            device,
            queue_family_index,
            pdevice,
            device_memory_properties,
            window,
            surface_loader,
            surface_format,
            present_queue,
            surface_resolution,
            swapchain_loader,
            swapchain,
            present_images,
            present_image_views,
            pool,
            draw_command_buffers,
            setup_command_buffer,
            app_setup_command_buffer,
            depth_image,
            depth_image_view,
            present_complete_semaphores,
            rendering_complete_semaphores,
            draw_commands_reuse_fences,
            surface,
            debug_call_back,
            debug_utils_loader,
            depth_image_memory,
            framebuffers,
            renderpass,
            pipeline_layout,
            descriptor_sets,
            graphic_pipeline,
            vertex_input_buffer,
            index_buffer,
            index_buffer_data,
            viewports,
            scissors,
            uniform_ptr,
            uniform_color_buffer_memory_req,
            i,
        })
        }
    }
}

pub fn world_matrix(pos: Vec3, rot: Vec3) -> Mat4 {
    let fov = 90.0f32.to_radians();
    let aspect_ratio = 800.0 / 600.0;
    let f = 1.0 / (fov / 2.0).tan();
    let near = 0.1;
    let far = 100.0;

    let translation_matrix = Mat4 {
        data: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [pos.x, pos.y, pos.z, 1.0],
        ]
    };

    let rotation_matrix_x = Mat4 {
        data: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, rot.x.cos(), rot.x.sin(), 0.0],
            [0.0, -rot.x.sin(), rot.x.cos(), 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ]
    };

    let rotation_matrix_y = Mat4 {
        data: [
            [rot.y.cos(), 0.0, -rot.y.sin(), 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [rot.y.sin(), 0.0, rot.y.cos(), 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ]
    };

    let rotation_matrix_z = Mat4 {
        data: [
            [rot.z.cos(), rot.z.sin(), 0.0, 0.0],
            [-rot.z.sin(), rot.z.cos(), 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ]
    };

    let rotation_matrix = mat4_mul(&rotation_matrix_z, &mat4_mul(&rotation_matrix_y, &rotation_matrix_x));

    let projection_matrix = Mat4 {
        data: [
            [f/aspect_ratio, 0.0, 0.0, 0.0],
            [0.0, f, 0.0, 0.0],
            [0.0, 0.0, -(far + near) / (near - far), 1.0],
            [0.0, 0.0, -2.0*far*near/(far-near), 0.0],
        ]
    };

    mat4_mul(&translation_matrix, &mat4_mul(&rotation_matrix, &projection_matrix))
}
