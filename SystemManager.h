#include <cstdarg>
#include <cstdint>
#include <cstdlib>
#include <ostream>
#include <new>

struct Handler;

struct Plugin;

struct WebSocket;

struct Header {
  const char *key;
  const char *value;
};

extern "C" {

const Plugin *register_plugin(const char *name, const Handler *methods);

intptr_t write_socket(WebSocket *socket, const char *data, uintptr_t len);

intptr_t read_socket(WebSocket *socket, char *buf, uintptr_t len);

void close_socket(WebSocket *socket);

const void *create_response(uint16_t status,
                            const Header *headers,
                            uintptr_t headers_len,
                            const char *body,
                            uintptr_t body_len);

}  // extern "C"
