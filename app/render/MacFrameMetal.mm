// SPDX-License-Identifier: MIT
#include "render/MacFrameMetal.hpp"

#include <Metal/Metal.h>
#include <mach/mach.h>
#include <mach/mach_vm.h>
#include <mach/vm_region.h>
#include <cstdint>
#include <unistd.h>

namespace mitcad::render {
struct MacFrameMetalBuffer::Impl {
  id<MTLBuffer> buffer = nil;
  ~Impl() { [buffer release]; }
};

MacFrameMetalBuffer::MacFrameMetalBuffer() : m_impl(std::make_unique<Impl>()) {}
MacFrameMetalBuffer::~MacFrameMetalBuffer() = default;

bool MacFrameMetalBuffer::supported() {
  @autoreleasepool {
    id<MTLDevice> device = MTLCreateSystemDefaultDevice();
    const bool result = device != nil && device.hasUnifiedMemory;
    [device release];
    return result;
  }
}

bool MacFrameMetalBuffer::wrap(IOSurfaceRef surface, std::string& error) {
  if (!surface || m_impl->buffer != nil) { error = "a Metal surface buffer requires a fresh owner"; return false; }
  void* base = IOSurfaceGetBaseAddress(surface);
  const auto length = IOSurfaceGetAllocSize(surface);
  const auto page = sysconf(_SC_PAGESIZE);
  if (!base || length == 0 || page <= 0 || reinterpret_cast<std::uintptr_t>(base) % std::size_t(page) != 0 ||
      length % std::size_t(page) != 0) {
    error = "the IOSurface is not a page-aligned Metal buffer allocation";
    return false;
  }
  // Apple's bytes-no-copy API requires the complete range to reside in one VM
  // region. Never round past the IOSurface's reported allocation to make it fit.
  mach_vm_address_t region = reinterpret_cast<mach_vm_address_t>(base);
  mach_vm_size_t regionSize = 0;
  vm_region_basic_info_data_64_t info{};
  mach_msg_type_number_t count = VM_REGION_BASIC_INFO_COUNT_64;
  mach_port_t object = MACH_PORT_NULL;
  const auto status = mach_vm_region(mach_task_self(), &region, &regionSize, VM_REGION_BASIC_INFO_64,
      reinterpret_cast<vm_region_info_t>(&info), &count, &object);
  if (MACH_PORT_VALID(object)) mach_port_deallocate(mach_task_self(), object);
  const auto address = reinterpret_cast<mach_vm_address_t>(base);
  if (status != KERN_SUCCESS || count != VM_REGION_BASIC_INFO_COUNT_64 ||
      (info.protection & (VM_PROT_READ | VM_PROT_WRITE)) != (VM_PROT_READ | VM_PROT_WRITE) ||
      address < region || address - region > regionSize ||
      length > regionSize - (address - region)) {
    error = "the IOSurface does not occupy a single supported VM region";
    return false;
  }
  @autoreleasepool {
    id<MTLDevice> device = MTLCreateSystemDefaultDevice();
    if (device == nil || !device.hasUnifiedMemory || length > device.maxBufferLength) {
      [device release];
      error = "the default Metal device cannot wrap this unified-memory surface";
      return false;
    }
    // The copied deallocator block owns the CF reference. Its lifetime follows
    // the MTLBuffer, including retains by in-flight GPU command buffers; failure
    // to create a buffer also releases the temporary owner exactly once.
    const auto retained = std::shared_ptr<const void>(CFRetain(surface), [](const void* value) { CFRelease(value); });
    m_impl->buffer = [device newBufferWithBytesNoCopy:base length:length options:MTLResourceStorageModeShared
        deallocator:^(void*, NSUInteger) { (void)retained; }];
    [device release];
    if (m_impl->buffer == nil) { error = "Metal rejected the IOSurface bytes-no-copy allocation"; return false; }
  }
  return true;
}

void* MacFrameMetalBuffer::handle() const { return (__bridge void*)m_impl->buffer; }
std::size_t MacFrameMetalBuffer::size() const { return m_impl->buffer != nil ? m_impl->buffer.length : 0; }
} // namespace mitcad::render
