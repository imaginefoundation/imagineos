#include <string.h>
#include <stdlib.h>

void *memcpy(void *destination, const void *source, size_t count)
{
    unsigned char *output = destination;
    const unsigned char *input = source;
    for (size_t index = 0; index < count; index++) output[index] = input[index];
    return destination;
}

void *memmove(void *destination, const void *source, size_t count)
{
    unsigned char *output = destination;
    const unsigned char *input = source;
    if ((unsigned long)output <= (unsigned long)input) {
        for (size_t index = 0; index < count; index++) output[index] = input[index];
    } else {
        while (count) {
            count--;
            output[count] = input[count];
        }
    }
    return destination;
}

void *memset(void *destination, int value, size_t count)
{
    unsigned char *output = destination;
    for (size_t index = 0; index < count; index++) output[index] = (unsigned char)value;
    return destination;
}

int memcmp(const void *left, const void *right, size_t count)
{
    const unsigned char *first = left;
    const unsigned char *second = right;
    for (size_t index = 0; index < count; index++) {
        if (first[index] != second[index]) return (int)first[index] - (int)second[index];
    }
    return 0;
}

size_t strlen(const char *text)
{
    size_t length = 0;
    while (text[length]) length++;
    return length;
}

int strcmp(const char *left, const char *right)
{
    while (*left && (unsigned char)*left == (unsigned char)*right) {
        left++;
        right++;
    }
    return (int)(unsigned char)*left - (int)(unsigned char)*right;
}

int strncmp(const char *left, const char *right, size_t count)
{
    for (size_t index = 0; index < count; index++) {
        unsigned char first = (unsigned char)left[index];
        unsigned char second = (unsigned char)right[index];
        if (first != second || first == 0) return (int)first - (int)second;
    }
    return 0;
}

char *strcpy(char *destination, const char *source)
{
    char *result = destination;
    while ((*destination++ = *source++)) {}
    return result;
}

char *strncpy(char *destination, const char *source, size_t count)
{
    size_t index = 0;
    while (index < count && source[index]) {
        destination[index] = source[index];
        index++;
    }
    while (index < count) destination[index++] = 0;
    return destination;
}

char *strchr(const char *text, int character)
{
    while (*text) {
        if ((unsigned char)*text == (unsigned char)character) return (char *)text;
        text++;
    }
    return character == 0 ? (char *)text : 0;
}

char *strstr(const char *text, const char *pattern)
{
    if (!*pattern) return (char *)text;
    size_t pattern_length = strlen(pattern);
    while (*text) {
        if (*text == *pattern && strncmp(text, pattern, pattern_length) == 0) {
            return (char *)text;
        }
        text++;
    }
    return 0;
}

char *strrchr(const char *text, int character)
{
    const char *result = 0;
    while (*text) {
        if ((unsigned char)*text == (unsigned char)character) result = text;
        text++;
    }
    return character == 0 ? (char *)text : (char *)result;
}

char *strpbrk(const char *text, const char *accept)
{
    for (const char *cursor = text; *cursor; cursor++) {
        for (const char *selector = accept; *selector; selector++) {
            if (*cursor == *selector) return (char *)cursor;
        }
    }
    return 0;
}

char *strcat(char *destination, const char *source)
{
    char *result = destination + strlen(destination);
    strcpy(result, source);
    return destination;
}

int strcasecmp(const char *left, const char *right)
{
    while (*left && *right) {
        unsigned char first = (unsigned char)*left;
        unsigned char second = (unsigned char)*right;
        if (first >= 'A' && first <= 'Z') first += 'a' - 'A';
        if (second >= 'A' && second <= 'Z') second += 'a' - 'A';
        if (first != second) return (int)first - (int)second;
        left++;
        right++;
    }
    return (int)(unsigned char)*left - (int)(unsigned char)*right;
}

char *strdup(const char *text)
{
    size_t length = strlen(text) + 1;
    char *copy = malloc(length);
    if (!copy) return 0;
    memcpy(copy, text, length);
    return copy;
}

size_t strcspn(const char *text, const char *reject)
{
    size_t count = 0;
    while (text[count]) {
        for (const char *cursor = reject; *cursor; cursor++) {
            if (text[count] == *cursor) return count;
        }
        count++;
    }
    return count;
}

size_t strspn(const char *text, const char *accept)
{
    size_t count = 0;
    while (text[count]) {
        int match = 0;
        for (const char *cursor = accept; *cursor; cursor++) {
            if (text[count] == *cursor) {
                match = 1;
                break;
            }
        }
        if (!match) break;
        count++;
    }
    return count;
}

char *strtok(char *text, const char *delimiters)
{
    static char *cursor;
    char *result;
    if (text) cursor = text;
    if (!cursor) return 0;
    while (*cursor && strchr(delimiters, *cursor)) cursor++;
    if (!*cursor) return (cursor = 0);
    result = cursor;
    while (*cursor && !strchr(delimiters, *cursor)) cursor++;
    if (*cursor) {
        *cursor = 0;
        cursor++;
    }
    return result;
}

char *strerror(int number)
{
    switch (number) {
    case 0: return "success";
    case 1: return "operation not permitted";
    case 2: return "no such file or directory";
    case 4: return "interrupted system call";
    case 5: return "input/output error";
    case 8: return "exec format error";
    case 9: return "bad file descriptor";
    case 11: return "resource temporarily unavailable";
    case 12: return "out of memory";
    case 13: return "permission denied";
    case 14: return "bad address";
    case 17: return "file exists";
    case 20: return "not a directory";
    case 21: return "is a directory";
    case 22: return "invalid argument";
    case 24: return "too many open files";
    case 28: return "no space left on device";
    case 29: return "illegal seek";
    case 34: return "result out of range";
    case 36: return "filename too long";
    case 38: return "function not implemented";
    case 39: return "directory not empty";
    case 75: return "value too large";
    default: return "unknown error";
    }
}

char *basename(char *path)
{
    char *last = strrchr(path, '/');
    return last ? last + 1 : path;
}

char *dirname(char *path)
{
    static char buffer[512];
    char *last = strrchr(path, '/');
    if (!last) {
        strcpy(buffer, ".");
        return buffer;
    }
    if (last == path) {
        strcpy(buffer, "/");
        return buffer;
    }
    size_t length = (size_t)(last - path);
    if (length >= sizeof(buffer)) length = sizeof(buffer) - 1;
    memcpy(buffer, path, length);
    buffer[length] = 0;
    return buffer;
}
