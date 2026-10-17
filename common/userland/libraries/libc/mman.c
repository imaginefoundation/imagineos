#include <errno.h>
#include <limits.h>
#include <string.h>
#include <sys/mman.h>
#include <unistd.h>

void *mmap(void *address, size_t length, int protection, int flags, int descriptor, long offset)
{
    (void)protection;
    (void)descriptor;
    (void)offset;
    if (address || !(flags & MAP_ANONYMOUS) || (flags & MAP_FIXED) || !length) {
        errno = 22;
        return MAP_FAILED;
    }
    if (length > (size_t)-1 - 4095 || length > (size_t)LONG_MAX - 4095) {
        errno = 12;
        return MAP_FAILED;
    }
    size_t rounded = (length + 4095) & ~(size_t)4095;
    void *memory = sbrk((long)rounded);
    if (memory == MAP_FAILED) return MAP_FAILED;
    memset(memory, 0, rounded);
    return memory;
}

int munmap(void *address, size_t length)
{
    if (!address || !length) {
        errno = 22;
        return -1;
    }
    return 0;
}

int mprotect(void *address, size_t length, int protection)
{
    (void)protection;
    void *end = sbrk(0);
    unsigned long start = (unsigned long)address;
    if (!address || !length || length > ULONG_MAX - start
        || start + length > (unsigned long)end) {
        errno = 22;
        return -1;
    }
    return 0;
}
