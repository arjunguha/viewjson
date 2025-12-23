#ifndef SLOPJSON_BRIDGING_HEADER_H
#define SLOPJSON_BRIDGING_HEADER_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

char *slopjson_parse_file(const char *path);
char *slopjson_parse_text(const char *content, const char *name);
void slopjson_string_free(char *ptr);

#ifdef __cplusplus
}
#endif

#endif /* SLOPJSON_BRIDGING_HEADER_H */
