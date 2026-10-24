// SPDX-License-Identifier: MIT
#pragma once

#include <Security/Security.h>
#include <xpc/xpc.h>
#include <string>
#include <vector>

namespace mitcad::render {
// A signed bundle's designated requirement also identifies an ad-hoc development
// binary by its code hash. XPC validates every message against this requirement.
inline bool requirePeer(xpc_connection_t connection, const char* bundle, std::string& error) {
  CFURLRef url = CFURLCreateFromFileSystemRepresentation(kCFAllocatorDefault,
      reinterpret_cast<const UInt8*>(bundle), static_cast<CFIndex>(std::char_traits<char>::length(bundle)), true);
  SecStaticCodeRef code = nullptr;
  SecRequirementRef requirement = nullptr;
  CFStringRef text = nullptr;
  OSStatus status = url ? SecStaticCodeCreateWithPath(url, kSecCSDefaultFlags, &code) : errSecParam;
  if (status == errSecSuccess) status = SecStaticCodeCheckValidity(code, kSecCSStrictValidate, nullptr);
  if (status == errSecSuccess) status = SecCodeCopyDesignatedRequirement(code, kSecCSDefaultFlags, &requirement);
  if (status == errSecSuccess) status = SecRequirementCopyString(requirement, kSecCSDefaultFlags, &text);
  bool valid = false;
  if (status == errSecSuccess) {
    const CFIndex bytes = CFStringGetMaximumSizeForEncoding(CFStringGetLength(text), kCFStringEncodingUTF8) + 1;
    std::vector<char> utf8(static_cast<std::size_t>(bytes));
    if (CFStringGetCString(text, utf8.data(), bytes, kCFStringEncodingUTF8)) {
      valid = xpc_connection_set_peer_code_signing_requirement(connection, utf8.data()) == 0;
    }
  }
  if (text) CFRelease(text);
  if (requirement) CFRelease(requirement);
  if (code) CFRelease(code);
  if (url) CFRelease(url);
  if (!valid) error = "could not require the embedded peer's valid designated signature: " + std::to_string(status);
  return valid;
}
} // namespace mitcad::render
