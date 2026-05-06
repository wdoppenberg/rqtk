# C++ Verification ID Checks (Compile-Time)

This setup mirrors Rust `#[verifies("...")]` behavior for C++ tests:

- verification IDs are discovered from `requirements/**/*.toml`
- a generated header exposes known IDs
- `VERIFIES("...")` fails compilation when the ID does not exist
- generation logic lives in Rust (`rqtk-cpp`) and is exposed via `cxx`

## 1) Generate Header During Build (CMake)

Add this to your `CMakeLists.txt`:

```cmake
cmake_minimum_required(VERSION 3.20)
project(my_cpp_project LANGUAGES CXX)

set(RQTK_REPO_ROOT "/absolute/path/to/rqtk") # or your local repo root
set(RQTK_GENERATED_DIR "${CMAKE_BINARY_DIR}/generated")
set(RQTK_VERIFICATION_HEADER "${RQTK_GENERATED_DIR}/rqtk_verification_ids.hpp")

add_custom_command(
  OUTPUT "${RQTK_VERIFICATION_HEADER}"
  COMMAND cargo run --quiet -p rqtk --
          --repo-root "${RQTK_REPO_ROOT}"
          codegen-cpp-verifies
          --output "${RQTK_VERIFICATION_HEADER}"
          --macro-name "VERIFIES"
  DEPENDS
    "${RQTK_REPO_ROOT}/requirements"
    "${RQTK_REPO_ROOT}/rqtk.toml"
  COMMENT "Generating rqtk_verification_ids.hpp from requirement verification activities"
  VERBATIM
)

add_custom_target(rqtk_verification_header DEPENDS "${RQTK_VERIFICATION_HEADER}")

add_executable(example_tests tests/example_tests.cpp)
add_dependencies(example_tests rqtk_verification_header)
target_include_directories(example_tests PRIVATE "${RQTK_GENERATED_DIR}")
target_compile_features(example_tests PRIVATE cxx_std_20)
```

If you want finer dependency tracking, append requirement TOML files to `DEPENDS`.

## 2) Use in C++ Tests

```cpp
#include "rqtk_verification_ids.hpp"

void lifecycle_round_trip() {
  VERIFIES("VA-SYS-001-01");
  // test body...
}
```

Compile fails when the activity ID does not exist in the requirements tree.

## 3) Optional Framework Metadata

For GoogleTest, you can pair compile-time validation with runtime tagging:

```cpp
#include "rqtk_verification_ids.hpp"
#include <gtest/gtest.h>

#define RQTK_GTEST_VERIFIES(ID_LITERAL) \
  VERIFIES(ID_LITERAL);                 \
  RecordProperty("rqtk_verifies", ID_LITERAL)

TEST(Lifecycle, RoundTrip) {
  RQTK_GTEST_VERIFIES("VA-SYS-001-01");
  EXPECT_TRUE(true);
}
```

This gives:

- compile-time safety (`VERIFIES`)
- report-visible traceability metadata (`RecordProperty`)

## 4) `cxx` Bridge Entry Point

The codegen crate exposes a `cxx` function:

- Rust crate: `rqtk-cpp`
- bridge namespace: `rqtk::codegen`
- function: `generate_verifies_header_to_file(repo_root, output, macro_name)`

This lets you invoke the same Rust codegen path from C++ tooling while reusing `rqtk-core` parsing logic.
