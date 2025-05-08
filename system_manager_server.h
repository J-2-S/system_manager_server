#include <cstdarg>
#include <cstdint>
#include <cstdlib>
#include <ostream>

#include <new>

struct Plugin;

using Callback = const char*(*)(uid_t, const char*, uintptr_t);

extern "C" {

void init_command(const Plugin *plugin,
                  const char *name,
                  Callback function,
                  bool needs_root,
                  bool takes_input);

void test_api();

}  // extern "C"
