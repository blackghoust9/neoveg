cmake_minimum_required(VERSION 3.16)

# Suppress modern CMake policy warnings in VS Code CMake Tools
if(POLICY CMP0091)
    cmake_policy(SET CMP0091 NEW)
endif()
if(POLICY CMP0042)
    cmake_policy(SET CMP0042 NEW)
endif()

project(neoveg_cpp_engine VERSION 2.1.0 LANGUAGES C CXX)

option(NEOVEG_ENABLE_SANITIZERS "Enable AddressSanitizer and UndefinedBehaviorSanitizer for QA builds" OFF)
option(NEOVEG_ENABLE_COVERAGE "Enable compiler coverage instrumentation for QA builds" OFF)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)
set(CMAKE_POSITION_INDEPENDENT_CODE ON)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

# Detect Target RID for automatic staging into Neoveg's runtimes/<rid>/native/ slot
if(WIN32)
    set(NEOVEG_RID "win-x64")
elseif(APPLE)
    if(CMAKE_SYSTEM_PROCESSOR MATCHES "arm64|aarch64")
        set(NEOVEG_RID "osx-arm64")
    else()
        set(NEOVEG_RID "osx-x64")
    endif()
elseif(UNIX)
    set(NEOVEG_RID "linux-x64")
else()
    message(FATAL_ERROR "Neoveg C++ Engine supports Windows, macOS, and Linux only.")
endif()

set(NEOVEG_THIRD_PARTY_DIR "${CMAKE_CURRENT_SOURCE_DIR}/../third_party")
set(NEOVEG_NATIVE_OUT "${CMAKE_CURRENT_SOURCE_DIR}/../runtimes/${NEOVEG_RID}/native")
set(NEOVEG_BUILD_RELEASE_OUT "${CMAKE_BINARY_DIR}/Release")

set(NEOVEG_CPP_SOURCES
    src/neoveg_cpp_engine.cpp
    whisper.cpp/src/neoveg_whisper_bridge.cpp
)

include(CTest)

set(NEOVEG_CPP_INCLUDE_DIRS
    "${CMAKE_CURRENT_SOURCE_DIR}/include"
    "${CMAKE_CURRENT_SOURCE_DIR}/whisper.cpp/include"
    "${NEOVEG_THIRD_PARTY_DIR}/openfx/include"
    "${NEOVEG_THIRD_PARTY_DIR}/vst3sdk/pluginterfaces"
    "${NEOVEG_THIRD_PARTY_DIR}/voukoder/include"
    "${NEOVEG_THIRD_PARTY_DIR}/whisper.cpp/include"
)

# Cross-platform shared library target: neoveg_cpp_engine (.dll / .so / .dylib)
add_library(neoveg_cpp_engine SHARED ${NEOVEG_CPP_SOURCES})

target_include_directories(neoveg_cpp_engine
    PUBLIC
        ${NEOVEG_CPP_INCLUDE_DIRS}
)

target_compile_definitions(neoveg_cpp_engine PRIVATE NEOVEG_CPP_EXPORTS)

if(WIN32)
    if(MSVC)
        target_compile_options(neoveg_cpp_engine PRIVATE /W4 /O2 /EHsc)
    else()
        target_compile_options(neoveg_cpp_engine PRIVATE -Wall -Wextra -O3)
    endif()
    target_compile_definitions(neoveg_cpp_engine PRIVATE WIN32_LEAN_AND_MEAN NOMINMAX)
    # DirectX is probed dynamically by the Windows diagnostic; no DirectX import libraries are required.
elseif(APPLE)
    target_compile_options(neoveg_cpp_engine PRIVATE -Wall -Wextra -O3)
    target_compile_definitions(neoveg_cpp_engine PRIVATE NEOVEG_PLATFORM_MACOS)
elseif(UNIX)
    target_compile_options(neoveg_cpp_engine PRIVATE -Wall -Wextra -O3)
    target_compile_definitions(neoveg_cpp_engine PRIVATE NEOVEG_PLATFORM_LINUX)
    target_link_libraries(neoveg_cpp_engine PRIVATE pthread)
    if(CMAKE_SYSTEM_NAME STREQUAL "Linux")
        target_link_libraries(neoveg_cpp_engine PRIVATE rt)
    endif()
else()
    message(FATAL_ERROR "No compiler configuration is defined for this platform.")
endif()

if(WIN32)
    set_target_properties(neoveg_cpp_engine PROPERTIES
        PREFIX ""
        OUTPUT_NAME "neoveg_cpp_engine"
        RUNTIME_OUTPUT_DIRECTORY "${NEOVEG_BUILD_RELEASE_OUT}"
        RUNTIME_OUTPUT_DIRECTORY_RELEASE "${NEOVEG_BUILD_RELEASE_OUT}"
        LIBRARY_OUTPUT_DIRECTORY "${NEOVEG_BUILD_RELEASE_OUT}"
        LIBRARY_OUTPUT_DIRECTORY_RELEASE "${NEOVEG_BUILD_RELEASE_OUT}"
        ARCHIVE_OUTPUT_DIRECTORY "${NEOVEG_BUILD_RELEASE_OUT}"
        ARCHIVE_OUTPUT_DIRECTORY_RELEASE "${NEOVEG_BUILD_RELEASE_OUT}"
    )
elseif(APPLE)
    set_target_properties(neoveg_cpp_engine PROPERTIES
        MACOSX_RPATH ON
        OUTPUT_NAME "neoveg_cpp_engine"
        LIBRARY_OUTPUT_DIRECTORY "${NEOVEG_BUILD_RELEASE_OUT}"
    )
elseif(UNIX)
    set_target_properties(neoveg_cpp_engine PROPERTIES
        OUTPUT_NAME "neoveg_cpp_engine"
        LIBRARY_OUTPUT_DIRECTORY "${NEOVEG_BUILD_RELEASE_OUT}"
    )
else()
    message(FATAL_ERROR "No shared-library output configuration is defined for this platform.")
endif()

if(NOT NEOVEG_ENABLE_SANITIZERS AND NOT NEOVEG_ENABLE_COVERAGE)
    add_custom_command(TARGET neoveg_cpp_engine POST_BUILD
        COMMAND ${CMAKE_COMMAND} -E make_directory "${NEOVEG_NATIVE_OUT}"
        COMMAND ${CMAKE_COMMAND} -E copy_if_different
            $<TARGET_FILE:neoveg_cpp_engine>
            "${NEOVEG_NATIVE_OUT}/$<TARGET_FILE_NAME:neoveg_cpp_engine>"
        COMMENT "Staging neoveg_cpp_engine dynamic binary into ${NEOVEG_NATIVE_OUT}"
    )
endif()

# Standalone whisper.cpp CLI executable (`neoveg-whisper-cli`) for out-of-process AI subtitle transcription
add_executable(neoveg_whisper_cli
    whisper.cpp/src/whisper_cli_main.cpp
    whisper.cpp/src/neoveg_whisper_bridge.cpp
)
target_include_directories(neoveg_whisper_cli PRIVATE ${NEOVEG_CPP_INCLUDE_DIRS})
set_target_properties(neoveg_whisper_cli PROPERTIES
    OUTPUT_NAME "neoveg-whisper-cli"
    RUNTIME_OUTPUT_DIRECTORY "${NEOVEG_BUILD_RELEASE_OUT}"
    RUNTIME_OUTPUT_DIRECTORY_RELEASE "${NEOVEG_BUILD_RELEASE_OUT}"
)

if(BUILD_TESTING)
    add_test(NAME neoveg_whisper_cli_smoke COMMAND neoveg_whisper_cli --duration 2.0)
endif()

if(NEOVEG_ENABLE_SANITIZERS OR NEOVEG_ENABLE_COVERAGE)
    if(NOT CMAKE_CXX_COMPILER_ID MATCHES "GNU|Clang"
       OR CMAKE_CXX_COMPILER_FRONTEND_VARIANT STREQUAL "MSVC")
        message(FATAL_ERROR "Neoveg QA sanitizers and coverage require GCC or a GNU-compatible Clang toolchain.")
    endif()

    foreach(qa_target IN ITEMS neoveg_cpp_engine neoveg_whisper_cli)
        if(NEOVEG_ENABLE_SANITIZERS)
            target_compile_options(${qa_target} PRIVATE -fsanitize=address,undefined -fno-omit-frame-pointer)
            target_link_options(${qa_target} PRIVATE -fsanitize=address,undefined)
        endif()

        if(NEOVEG_ENABLE_COVERAGE)
            target_compile_options(${qa_target} PRIVATE --coverage)
            target_link_options(${qa_target} PRIVATE --coverage)
        endif()
    endforeach()
endif()

if(NOT NEOVEG_ENABLE_SANITIZERS AND NOT NEOVEG_ENABLE_COVERAGE)
    add_custom_command(TARGET neoveg_whisper_cli POST_BUILD
        COMMAND ${CMAKE_COMMAND} -E make_directory "${NEOVEG_NATIVE_OUT}"
        COMMAND ${CMAKE_COMMAND} -E copy_if_different
            $<TARGET_FILE:neoveg_whisper_cli>
            "${NEOVEG_NATIVE_OUT}/$<TARGET_FILE_NAME:neoveg_whisper_cli>"
        COMMENT "Staging neoveg-whisper-cli into ${NEOVEG_NATIVE_OUT}"
    )
endif()
