// SPDX-License-Identifier: MIT
// The macOS update's filesystem transaction. Both directories already
// exist on one volume: RENAME_SWAP exchanges their names atomically.
#include <cerrno>
#include <cstdio>
#include <cstring>
#include <sys/stdio.h>

int main(int argc, char* argv[]) {
  if (argc != 4 || std::strcmp(argv[1], "--swap") != 0) {
    std::fputs("usage: mitcad-updater --swap <installed bundle> <staged bundle>\n", stderr);
    return 2;
  }
  if (renamex_np(argv[2], argv[3], RENAME_SWAP) != 0) {
    std::fprintf(stderr, "Update failed: cannot atomically exchange application bundles: %s\n", std::strerror(errno));
    return 1;
  }
  return 0;
}
