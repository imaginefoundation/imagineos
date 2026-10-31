#include <errno.h>
#include <sys/time.h>
#include <time.h>

static struct tm time_result;

time_t time(time_t *result)
{
    struct timeval value;
    if (gettimeofday(&value, 0) != 0) return (time_t)-1;
    if (result) *result = value.tv_sec;
    return value.tv_sec;
}

clock_t clock(void)
{
    struct timeval value;
    if (gettimeofday(&value, 0) != 0) return (clock_t)-1;
    return (clock_t)value.tv_sec * CLOCKS_PER_SEC + value.tv_usec;
}

double difftime(time_t end, time_t beginning)
{
    return (double)(end - beginning);
}

int nanosleep(const struct timespec *request, struct timespec *remaining)
{
    (void)request;
    if (remaining) {
        remaining->tv_sec = 0;
        remaining->tv_nsec = 0;
    }
    errno = 38;
    return -1;
}

struct tm *gmtime_r(const time_t *value, struct tm *result)
{
    static const unsigned char month_days[] = {31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31};
    if (!value || !result || *value < 0) {
        errno = 22;
        return 0;
    }
    unsigned long long seconds = (unsigned long long)*value;
    unsigned long long days = seconds / 86400;
    unsigned long long day_seconds = seconds % 86400;
    int year = 1970;
    for (;;) {
        int leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
        unsigned int year_days = leap ? 366U : 365U;
        if (days < year_days) break;
        days -= year_days;
        year++;
    }
    int leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    unsigned int month = 0;
    while (month < 11) {
        unsigned int count = month_days[month] + (month == 1 && leap);
        if (days < count) break;
        days -= count;
        month++;
    }
    result->tm_sec = (int)(day_seconds % 60);
    result->tm_min = (int)((day_seconds / 60) % 60);
    result->tm_hour = (int)(day_seconds / 3600);
    result->tm_mday = (int)days + 1;
    result->tm_mon = (int)month;
    result->tm_year = year - 1900;
    result->tm_wday = (int)(((seconds / 86400) + 4) % 7);
    result->tm_yday = (int)(seconds / 86400 - (unsigned long long)(
        (year - 1970) * 365 + (year - 1969) / 4 - (year - 1901) / 100 + (year - 1601) / 400));
    result->tm_isdst = 0;
    return result;
}

struct tm *localtime_r(const time_t *value, struct tm *result)
{
    return gmtime_r(value, result);
}

struct tm *gmtime(const time_t *value)
{
    return gmtime_r(value, &time_result);
}

struct tm *localtime(const time_t *value)
{
    return localtime_r(value, &time_result);
}
