// Bouchon freestanding pour compiler minimp3 (crate rmp3) en cross-target
// sans sysroot Linux. Les symboles memcpy / memmove / memset / memcmp
// sont fournis par le noyau (src/lib.rs).
#ifndef NOS_SHIM_STRING_H
#define NOS_SHIM_STRING_H
#include <stddef.h>
void *memcpy(void *, const void *, size_t);
void *memmove(void *, const void *, size_t);
void *memset(void *, int, size_t);
int memcmp(const void *, const void *, size_t);
#endif
