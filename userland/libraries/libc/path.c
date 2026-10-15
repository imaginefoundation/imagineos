#include <stdlib.h>
#include <string.h>
#include "dreamcore.h"
#include <unistd.h>

int dc_resolve_path(const char *path, char *output, size_t capacity)
{
    if (!path || !output || capacity == 0 || !path[0]) {
        errno = 22;
        return -1;
    }

    const char *working_directory = getenv("PWD");
    if (!working_directory || working_directory[0] != '/') working_directory = "/";
    const char *base = path[0] == '/' ? "" : working_directory;
    size_t base_length = strlen(base);
    size_t path_length = strlen(path);
    char joined[512];
    size_t joined_length = 0;
    if (base_length + path_length + 2 > sizeof(joined)) {
        errno = 36;
        return -1;
    }
    if (base_length) {
        memcpy(joined, base, base_length);
        joined_length = base_length;
    }
    if (joined_length == 0 || joined[joined_length - 1] != '/') {
        joined[joined_length++] = '/';
    }
    memcpy(joined + joined_length, path, path_length);
    joined_length += path_length;

    if (capacity < 2) {
        errno = 36;
        return -1;
    }
    output[0] = '/';
    size_t output_length = 1;
    size_t component_starts[128];
    size_t component_count = 0;
    size_t position = 0;
    while (position < joined_length) {
        while (position < joined_length && joined[position] == '/') position++;
        size_t start = position;
        while (position < joined_length && joined[position] != '/') position++;
        size_t length = position - start;
        if (length == 0 || (length == 1 && joined[start] == '.')) continue;
        if (length == 2 && joined[start] == '.' && joined[start + 1] == '.') {
            if (component_count) {
                size_t component_start = component_starts[--component_count];
                output_length = component_start > 1 ? component_start - 1 : 1;
            }
            continue;
        }
        if (component_count == sizeof(component_starts) / sizeof(component_starts[0])) {
            errno = 36;
            return -1;
        }
        size_t separator = output_length > 1;
        if (output_length + separator + length + 1 > capacity) {
            errno = 36;
            return -1;
        }
        if (separator) output[output_length++] = '/';
        component_starts[component_count++] = output_length;
        memcpy(output + output_length, joined + start, length);
        output_length += length;
    }
    output[output_length] = 0;
    return (int)output_length;
}
