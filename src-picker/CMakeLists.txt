cmake_minimum_required(VERSION 3.16)
project(omnirec-picker LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)

# Find Qt6 components
find_package(Qt6 REQUIRED COMPONENTS Widgets Network)

# Source files
set(SOURCES
    main.cpp
    ipc_client.cpp
    picker_logic.cpp
    dialog.cpp
)

set(HEADERS
    ipc_client.h
    picker_logic.h
    dialog.h
)

# Create executable
add_executable(omnirec-picker ${SOURCES} ${HEADERS})

# Link Qt libraries
target_link_libraries(omnirec-picker PRIVATE
    Qt6::Widgets
    Qt6::Network
)

# Compiler warnings
target_compile_options(omnirec-picker PRIVATE
    -Wall
    -Wextra
    -Wpedantic
)

# Install target
install(TARGETS omnirec-picker RUNTIME DESTINATION bin)
