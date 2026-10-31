#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <unistd.h>
#include <limits.h>
#include <math.h>
#include <stdint.h>
#include "dreamcore.h"

typedef union MemoryBlock MemoryBlock;
union MemoryBlock {
    struct {
        size_t size;
        MemoryBlock *next;
        int free;
    } allocation;
    long double floating;
    void *pointer;
};

#define HEAP_ALIGNMENT _Alignof(MemoryBlock)
#define HEAP_CHUNK_SIZE (64 * 1024)

static MemoryBlock *blocks;
char **environ;

void *malloc(size_t size)
{
    if (size == 0) size = 1;
    if (size > (size_t)-1 - (HEAP_ALIGNMENT - 1)) {
        errno = 12;
        return 0;
    }
    size = (size + HEAP_ALIGNMENT - 1) & ~(HEAP_ALIGNMENT - 1);
    for (MemoryBlock *block = blocks; block; block = block->allocation.next) {
        if (block->allocation.free && block->allocation.size >= size) {
            block->allocation.free = 0;
            return block + 1;
        }
    }

    if (size > (size_t)LONG_MAX - sizeof(MemoryBlock)) {
        errno = 12;
        return 0;
    }
    size_t capacity = size + sizeof(MemoryBlock);
    if (capacity < HEAP_CHUNK_SIZE) capacity = HEAP_CHUNK_SIZE;
    long increment = (long)capacity;
    void *memory = (void *)dc_sbrk(increment);
    if (memory == (void *)-1) {
        errno = 12;
        return 0;
    }
    MemoryBlock *block = memory;
    block->allocation.size = capacity - sizeof(*block);
    block->allocation.next = blocks;
    block->allocation.free = 0;
    blocks = block;
    return block + 1;
}

void *calloc(size_t count, size_t size)
{
    if (size && count > (size_t)-1 / size) return 0;
    size_t total = count * size;
    void *allocation = malloc(total);
    if (allocation) memset(allocation, 0, total);
    return allocation;
}

void *realloc(void *pointer, size_t size)
{
    if (!pointer) return malloc(size);
    if (size == 0) {
        free(pointer);
        return 0;
    }
    MemoryBlock *header = (MemoryBlock *)pointer - 1;
    if (header->allocation.size >= size) return pointer;
    void *replacement = malloc(size);
    if (!replacement) return 0;
    size_t copied = header->allocation.size < size ? header->allocation.size : size;
    memcpy(replacement, pointer, copied);
    free(pointer);
    return replacement;
}

void free(void *pointer)
{
    if (pointer) ((MemoryBlock *)pointer - 1)->allocation.free = 1;
}

int abs(int value)
{
    return value < 0 ? -value : value;
}

_Noreturn void abort(void)
{
    _exit(134);
}

// Weak para permitir que o `runmain.o` do TinyCC (modo `tcc -run`)
// forneça a sua própria `exit`, que retorna ao compilador em vez de
// terminar o processo. Sem `runmain.o`, esta definição é usada normalmente.
__attribute__((weak))
_Noreturn void exit(int status)
{
    _exit(status);
}

int atoi(const char *text)
{
    int value = 0;
    int sign = 1;
    if (*text == '-') {
        sign = -1;
        text++;
    }
    while (*text >= '0' && *text <= '9') {
        value = value * 10 + (*text - '0');
        text++;
    }
    return value * sign;
}

static unsigned digit_value(unsigned char character)
{
    if (character >= '0' && character <= '9') return character - '0';
    if (character >= 'a' && character <= 'z') return character - 'a' + 10;
    if (character >= 'A' && character <= 'Z') return character - 'A' + 10;
    return 36;
}

static unsigned long long parse_unsigned(
    const char *text, char **endptr, int base, unsigned long long maximum, int *negative)
{
    const char *original = text;
    while (*text == ' ' || (*text >= '\t' && *text <= '\r')) text++;
    *negative = 0;
    if (*text == '-' || *text == '+') {
        *negative = *text == '-';
        text++;
    }
    if (base != 0 && (base < 2 || base > 36)) {
        errno = 22;
        if (endptr) *endptr = (char *)original;
        return 0;
    }
    if ((base == 0 || base == 16) && text[0] == '0'
        && (text[1] == 'x' || text[1] == 'X')
        && digit_value((unsigned char)text[2]) < 16) {
        text += 2;
        base = 16;
    } else if (base == 0) {
        base = text[0] == '0' ? 8 : 10;
    }
    const char *digits = text;
    unsigned long long value = 0;
    int overflow = 0;
    while (*text) {
        unsigned digit = digit_value((unsigned char)*text);
        if (digit >= (unsigned)base) break;
        if (value > (maximum - digit) / (unsigned)base) {
            value = maximum;
            overflow = 1;
        } else if (!overflow) {
            value = value * (unsigned)base + digit;
        }
        text++;
    }
    if (text == digits) {
        if (endptr) *endptr = (char *)original;
        return 0;
    }
    if (endptr) *endptr = (char *)text;
    if (overflow) errno = 34;
    return value;
}

unsigned long strtoul(const char *text, char **endptr, int base)
{
    int negative;
    unsigned long long value = parse_unsigned(text, endptr, base, ULONG_MAX, &negative);
    return negative ? (unsigned long)(0UL - (unsigned long)value) : (unsigned long)value;
}

unsigned long long strtoull(const char *text, char **endptr, int base)
{
    int negative;
    unsigned long long value = parse_unsigned(text, endptr, base, ULLONG_MAX, &negative);
    return negative ? 0ULL - value : value;
}

long strtol(const char *text, char **endptr, int base)
{
    int negative;
    unsigned long long limit = (unsigned long long)LONG_MAX + 1ULL;
    unsigned long long value = parse_unsigned(text, endptr, base, limit, &negative);
    if (value > (negative ? limit : (unsigned long long)LONG_MAX)) {
        errno = 34;
        return negative ? LONG_MIN : LONG_MAX;
    }
    if (negative) return value == limit ? LONG_MIN : -(long)value;
    return (long)value;
}

long long strtoll(const char *text, char **endptr, int base)
{
    int negative;
    unsigned long long limit = (unsigned long long)LLONG_MAX + 1ULL;
    unsigned long long value = parse_unsigned(text, endptr, base, limit, &negative);
    if (value > (negative ? limit : (unsigned long long)LLONG_MAX)) {
        errno = 34;
        return negative ? LLONG_MIN : LLONG_MAX;
    }
    if (negative) return value == limit ? LLONG_MIN : -(long long)value;
    return (long long)value;
}

intmax_t strtoimax(const char *text, char **endptr, int base)
{
    return (intmax_t)strtoll(text, endptr, base);
}

uintmax_t strtoumax(const char *text, char **endptr, int base)
{
    return (uintmax_t)strtoull(text, endptr, base);
}

double strtod(const char *text, char **endptr)
{
    const char *original = text;
    while (*text == ' ' || (*text >= '\t' && *text <= '\r')) text++;
    int negative = 0;
    if (*text == '-' || *text == '+') {
        negative = *text == '-';
        text++;
    }
    int hexadecimal = text[0] == '0' && (text[1] == 'x' || text[1] == 'X');
    if (hexadecimal) text += 2;
    long double value = 0.0L;
    long double scale = hexadecimal ? 1.0L : 0.1L;
    unsigned base = hexadecimal ? 16 : 10;
    int saw_digit = 0;
    while (digit_value((unsigned char)*text) < base) {
        saw_digit = 1;
        value = value * base + digit_value((unsigned char)*text++);
    }
    if (*text == '.') {
        text++;
        while (digit_value((unsigned char)*text) < base) {
            saw_digit = 1;
            value += digit_value((unsigned char)*text++) * scale;
            scale /= base;
        }
    }
    if (!saw_digit) {
        if (endptr) *endptr = (char *)original;
        return 0.0;
    }
    int exponent = 0;
    if ((!hexadecimal && (*text == 'e' || *text == 'E'))
        || (hexadecimal && (*text == 'p' || *text == 'P'))) {
        const char *marker = text++;
        int exponent_negative = 0;
        if (*text == '-' || *text == '+') {
            exponent_negative = *text == '-';
            text++;
        }
        if (*text < '0' || *text > '9') {
            text = marker;
        } else {
            while (*text >= '0' && *text <= '9') {
                if (exponent < 10000) exponent = exponent * 10 + (*text - '0');
                text++;
            }
            if (exponent_negative) exponent = -exponent;
        }
    }
    if (hexadecimal) {
        while (exponent > 0) { value *= 2.0L; exponent--; }
        while (exponent < 0) { value *= 0.5L; exponent++; }
    } else {
        while (exponent > 0 && exponent <= 308) { value *= 10.0L; exponent--; }
        while (exponent < 0 && exponent >= -308) { value *= 0.1L; exponent++; }
        if (exponent > 0) value = HUGE_VAL;
        if (exponent < 0) value = 0.0L;
    }
    if (endptr) *endptr = (char *)text;
    return (double)(negative ? -value : value);
}

float strtof(const char *text, char **endptr)
{
    return (float)strtod(text, endptr);
}

long double strtold(const char *text, char **endptr)
{
    return (long double)strtod(text, endptr);
}

char *getenv(const char *name)
{
    if (!name || !environ) return 0;
    size_t length = strlen(name);
    for (char **entry = environ; *entry; entry++) {
        if (strncmp(*entry, name, length) == 0 && (*entry)[length] == '=') {
            return *entry + length + 1;
        }
    }
    return 0;
}

int putenv(const char *string)
{
    (void)string;
    errno = 38;
    return -1;
}

int unsetenv(const char *name)
{
    (void)name;
    errno = 38;
    return -1;
}

int setenv(const char *name, const char *value, int overwrite)
{
    (void)name;
    (void)value;
    (void)overwrite;
    errno = 38;
    return -1;
}

void *__errno_location(void)
{
    return &errno;
}

int *__errno(void)
{
    return (int *)__errno_location();
}

int sem_init(void *sem, int pshared, unsigned int value)
{
    (void)sem;
    (void)pshared;
    (void)value;
    errno = 38;
    return -1;
}

int sem_wait(void *sem)
{
    (void)sem;
    errno = 38;
    return -1;
}

int sem_post(void *sem)
{
    (void)sem;
    errno = 38;
    return -1;
}

_Noreturn void __assert_fail(
    const char *expression, const char *file, unsigned int line, const char *function)
{
    fprintf(stderr, "%s:%u: %s: Assertion `%s` failed\n", file, line, function, expression);
    abort();
}

void __stack_chk_fail(void)
{
    abort();
}

void *realpath(const char *path, char *resolved_path)
{
    char resolved[256];
    int length = dc_resolve_path(path, resolved, sizeof(resolved));
    if (length < 0) return 0;
    if (!resolved_path) {
        resolved_path = malloc((size_t)length + 1);
        if (!resolved_path) return 0;
    }
    memcpy(resolved_path, resolved, (size_t)length + 1);
    return resolved_path;
}

unsigned long long __strtoull_internal(const char *text, char **endptr, int base)
{
    return strtoull(text, endptr, base);
}

unsigned long __strtoul_internal(const char *text, char **endptr, int base)
{
    return strtoul(text, endptr, base);
}

long __isoc23_strtol(const char *text, char **endptr, int base)
{
    return strtol(text, endptr, base);
}

unsigned long __isoc23_strtoul(const char *text, char **endptr, int base)
{
    return strtoul(text, endptr, base);
}

unsigned long long __isoc23_strtoull(const char *text, char **endptr, int base)
{
    return strtoull(text, endptr, base);
}

long __strtol_internal(const char *text, char **endptr, int base, int group)
{
    (void)group;
    return strtol(text, endptr, base);
}

long long __strtoll_internal(const char *text, char **endptr, int base, int group)
{
    (void)group;
    return strtoll(text, endptr, base);
}

char *strndup(const char *text, size_t length)
{
    char *copy = malloc(length + 1);
    if (!copy) return 0;
    memcpy(copy, text, length);
    copy[length] = 0;
    return copy;
}

int vasprintf(char **buffer, const char *format, va_list arguments)
{
    va_list copy;
    va_copy(copy, arguments);
    int result = vsnprintf(0, 0, format, copy);
    va_end(copy);
    if (result < 0) return result;
    *buffer = malloc((size_t)result + 1U);
    if (!*buffer) return -1;
    va_copy(copy, arguments);
    int written = vsnprintf(*buffer, (size_t)result + 1U, format, copy);
    va_end(copy);
    if (written != result) {
        free(*buffer);
        *buffer = 0;
        return -1;
    }
    return result;
}

int asprintf(char **buffer, const char *format, ...)
{
    va_list arguments;
    va_start(arguments, format);
    int result = vasprintf(buffer, format, arguments);
    va_end(arguments);
    return result;
}

char *index(const char *text, int character)
{
    return strchr(text, character);
}

char *rindex(const char *text, int character)
{
    return strrchr(text, character);
}

char *strcasestr(const char *haystack, const char *needle)
{
    size_t needle_length = strlen(needle);
    if (needle_length == 0) return (char *)haystack;
    for (size_t index = 0; haystack[index]; index++) {
        if (strncasecmp(haystack + index, needle, needle_length) == 0) return (char *)(haystack + index);
    }
    return 0;
}

int strncasecmp(const char *left, const char *right, size_t count)
{
    for (size_t index = 0; index < count; index++) {
        unsigned char first = (unsigned char)left[index];
        unsigned char second = (unsigned char)right[index];
        if (!first || !second) return (int)first - (int)second;
        if (first >= 'A' && first <= 'Z') first += 'a' - 'A';
        if (second >= 'A' && second <= 'Z') second += 'a' - 'A';
        if (first != second) return (int)first - (int)second;
    }
    return 0;
}

void *bsearch(const void *key, const void *base, size_t nmemb, size_t size, int (*compar)(const void *, const void *))
{
    const unsigned char *items = base;
    size_t low = 0, high = nmemb;
    while (low < high) {
        size_t middle = low + (high - low) / 2;
        const void *item = items + middle * size;
        int order = compar(key, item);
        if (order == 0) return (void *)item;
        if (order < 0) high = middle;
        else low = middle + 1;
    }
    return 0;
}

static void swap_elements(unsigned char *left, unsigned char *right, size_t size)
{
    for (size_t index = 0; index < size; index++) {
        unsigned char temporary = left[index];
        left[index] = right[index];
        right[index] = temporary;
    }
}

static void sift_down(
    unsigned char *items, size_t root, size_t end, size_t size,
    int (*compar)(const void *, const void *))
{
    while (root < end / 2) {
        size_t child = root * 2 + 1;
        if (child + 1 < end
            && compar(items + child * size, items + (child + 1) * size) < 0) {
            child++;
        }
        if (compar(items + root * size, items + child * size) >= 0) return;
        swap_elements(items + root * size, items + child * size, size);
        root = child;
    }
}

void qsort(void *base, size_t nmemb, size_t size, int (*compar)(const void *, const void *))
{
    if (!base || !compar || size == 0 || nmemb < 2) return;
    unsigned char *items = base;
    for (size_t start = nmemb / 2; start > 0; start--) {
        sift_down(items, start - 1, nmemb, size, compar);
    }
    for (size_t end = nmemb; end > 1; end--) {
        swap_elements(items, items + (end - 1) * size, size);
        sift_down(items, 0, end - 1, size, compar);
    }
}
