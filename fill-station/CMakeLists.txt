cmake_minimum_required(VERSION 3.20)
project(fill-station LANGUAGES CXX)
set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

find_program(GRPC_CPP_PLUGIN grpc_cpp_plugin REQUIRED)
find_package(spdlog REQUIRED)
find_package(gRPC CONFIG REQUIRED)
find_package(PkgConfig REQUIRED)
pkg_check_modules(LIBGPIODCXX REQUIRED IMPORTED_TARGET libgpiodcxx)

set(PROTO_BINARY_DIR "${CMAKE_CURRENT_BINARY_DIR}/proto")
file(MAKE_DIRECTORY ${PROTO_BINARY_DIR})

add_executable(fill-station
  src/main.cpp
  src/hardware.cpp
  src/igniter.cpp
  src/command_service.cpp
  command.proto
)
target_include_directories(fill-station PRIVATE
  include
  ${PROTO_BINARY_DIR}
)
target_link_libraries(fill-station PRIVATE
  gRPC::grpc++
  spdlog::spdlog
  PkgConfig::LIBGPIODCXX
)
protobuf_generate(
  TARGET fill-station
  LANGUAGE cpp
  PROTOC_OUT_DIR ${PROTO_BINARY_DIR}
)
protobuf_generate(
  TARGET fill-station
  LANGUAGE grpc
  GENERATE_EXTENSIONS .grpc.pb.h .grpc.pb.cc
  PROTOC_OUT_DIR ${PROTO_BINARY_DIR}
  PLUGIN "protoc-gen-grpc=${GRPC_CPP_PLUGIN}"
)

if(MSVC)
  target_compile_options(fill-station PRIVATE /W4 /WX)
else()
  target_compile_options(fill-station PRIVATE -Wall -Wextra -Wpedantic)
endif()

install(TARGETS fill-station RUNTIME DESTINATION bin)