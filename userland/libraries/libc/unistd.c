#include <unistd.h>
#include <fcntl.h>
#include <string.h>
#include <stdarg.h>
#include <stdlib.h>
#include <sys/stat.h>
#include <sys/time.h>

#include "dreamcore.h"

int errno;

static ssize_t set_error(int error)
{
    errno = error;
    return -1;
}

ssize_t write(int descriptor, const void *buffer, size_t count)
{
    if (count > 4096) count = 4096;
    long result = dc_write_fd(descriptor, buffer, count);
    if (result < 0) {
        return set_error((int)-result);
    }
    return result;
}

ssize_t read(int descriptor, void *buffer, size_t count)
{
    if (count > 4096) count = 4096;
    long result = dc_read_fd(descriptor, buffer, count);
    return result < 0 ? set_error((int)-result) : result;
}

int close(int descriptor)
{
    long result = dc_close(descriptor);
    return result < 0 ? (int)set_error((int)-result) : (int)result;
}

off_t lseek(int descriptor, off_t offset, int whence)
{
    long result = dc_lseek(descriptor, offset, whence);
    if (result < 0) {
        errno = (int)-result;
        return (off_t)-1;
    }
    return (off_t)result;
}

void *sbrk(long increment)
{
    long result = dc_sbrk(increment);
    if (result < 0) {
        errno = (int)-result;
        return (void *)-1;
    }
    return (void *)result;
}

int open(const char *path, int flags, ...)
{
    unsigned long runtime_flags = 0;
    switch (flags & O_ACCMODE) {
    case O_RDONLY:
        runtime_flags |= DC_OPEN_READ;
        break;
    case O_WRONLY:
        runtime_flags |= DC_OPEN_WRITE;
        break;
    case O_RDWR:
        runtime_flags |= DC_OPEN_READ | DC_OPEN_WRITE;
        break;
    default:
        errno = 22;
        return -1;
    }
    if (flags & O_CREAT) runtime_flags |= DC_OPEN_CREATE;
    if (flags & O_TRUNC) runtime_flags |= DC_OPEN_TRUNCATE;
    if (flags & O_APPEND) runtime_flags |= DC_OPEN_APPEND;
    if (flags & O_EXCL) runtime_flags |= 32;
    if (flags & ~(O_ACCMODE | O_CREAT | O_EXCL | O_TRUNC | O_APPEND)) {
        errno = 22;
        return -1;
    }
    char resolved[256];
    if (dc_resolve_path(path, resolved, sizeof(resolved)) < 0) return -1;
    va_list arguments;
    va_start(arguments, flags);
    unsigned long mode = flags & O_CREAT ? (unsigned long)va_arg(arguments, int) : 0;
    va_end(arguments);
    long result = dc_open(resolved, dc_strlen(resolved), (int)runtime_flags, mode);
    if (result < 0) {
        errno = (int)-result;
        return -1;
    }
    return (int)result;
}

int stat(const char *path, struct stat *buffer)
{
    if (!buffer) {
        errno = 22;
        return -1;
    }
    struct {
        unsigned long long size;
        unsigned int mode;
        unsigned int uid;
        unsigned int gid;
        unsigned int kind;
    } metadata;
    char resolved[256];
    if (dc_resolve_path(path, resolved, sizeof(resolved)) < 0) return -1;
    long result = dc_syscall3(
        DC_SYS_STAT, (long)resolved, (long)dc_strlen(resolved), (long)&metadata);
    if (result < 0) {
        errno = (int)-result;
        return -1;
    }
    memset(buffer, 0, sizeof(*buffer));
    buffer->st_mode = metadata.mode | (metadata.kind ? S_IFDIR : S_IFREG);
    buffer->st_uid = metadata.uid;
    buffer->st_gid = metadata.gid;
    buffer->st_size = (long long)metadata.size;
    buffer->st_blksize = 512;
    buffer->st_blocks = (long long)((metadata.size + 511) / 512);
    return 0;
}

int fstat(int descriptor, struct stat *buffer)
{
    if (!buffer) {
        errno = 22;
        return -1;
    }
    struct {
        unsigned long long size;
        unsigned int mode;
        unsigned int uid;
        unsigned int gid;
        unsigned int kind;
    } metadata;
    long result = dc_syscall2(DC_SYS_FSTAT, descriptor, (long)&metadata);
    if (result < 0) {
        errno = (int)-result;
        return -1;
    }
    memset(buffer, 0, sizeof(*buffer));
    buffer->st_mode = metadata.mode | (metadata.kind ? S_IFDIR : S_IFREG);
    buffer->st_uid = metadata.uid;
    buffer->st_gid = metadata.gid;
    buffer->st_size = (long long)metadata.size;
    buffer->st_blksize = 512;
    buffer->st_blocks = (long long)((metadata.size + 511) / 512);
    return 0;
}

void _exit(int status)
{
    dc_exit(status);
}

int unlink(const char *path)
{
    char resolved[256];
    if (dc_resolve_path(path, resolved, sizeof(resolved)) < 0) return -1;
    long result = dc_syscall3(14, (long)resolved, (long)dc_strlen(resolved), 0);
    if (result < 0) {
        errno = (int)-result;
        return -1;
    }
    return 0;
}

int remove(const char *path)
{
    return unlink(path);
}

int execvp(const char *file, char *const arguments[])
{
    (void)file;
    (void)arguments;
    errno = 38;
    return -1;
}

int access(const char *path, int mode)
{
    struct stat metadata;
    if (stat(path, &metadata) != 0) return -1;
    if (mode == F_OK) return 0;
    if (mode & ~(R_OK | W_OK | X_OK)) {
        errno = 22;
        return -1;
    }
    unsigned int requested = (mode & R_OK ? 4U : 0U)
        | (mode & W_OK ? 2U : 0U)
        | (mode & X_OK ? 1U : 0U);
    struct {
        unsigned int uid;
        unsigned int gid;
        unsigned int is_admin;
        unsigned int username_length;
        char username[32];
        unsigned int hostname_length;
        char hostname[64];
    } identity;
    long result = dc_syscall1(DC_SYS_GETIDENTITY, (long)&identity);
    if (result < 0) {
        errno = (int)-result;
        return -1;
    }
    if (identity.uid == 0) return 0;
    unsigned int permissions = (metadata.st_mode >> 6) & 7U;
    if (identity.uid != metadata.st_uid) {
        permissions = identity.gid == metadata.st_gid
            ? ((metadata.st_mode >> 3) & 7U)
            : (metadata.st_mode & 7U);
    }
    if ((permissions & requested) != requested) {
        errno = 13;
        return -1;
    }
    return 0;
}

char *getcwd(char *buffer, size_t capacity)
{
    const char *working_directory = getenv("PWD");
    if (!working_directory || !*working_directory) working_directory = "/";
    char resolved[256];
    int length = dc_resolve_path(working_directory, resolved, sizeof(resolved));
    if (length < 0) return 0;
    if (!buffer) {
        buffer = malloc((size_t)length + 1);
        if (!buffer) return 0;
        capacity = (size_t)length + 1;
    }
    if (capacity <= (size_t)length) {
        errno = 34;
        return 0;
    }
    memcpy(buffer, resolved, (size_t)length + 1);
    return buffer;
}

int gettimeofday(struct timeval *value, void *timezone)
{
    (void)timezone;
    long result = dc_syscall1(DC_SYS_GETTIMEOFDAY, (long)value);
    if (result < 0) {
        errno = (int)-result;
        return -1;
    }
    return 0;
}