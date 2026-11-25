cmake_minimum_required(VERSION 3.16)
project(diyardesk_plasma_gl LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)

find_package(Qt6 REQUIRED COMPONENTS Core Gui Qml Quick OpenGL)

add_library(diyardesk_plasma_gl MODULE
    diyardesk_gl_item.h
    diyardesk_gl_item.cpp
)

set_target_properties(diyardesk_plasma_gl PROPERTIES
    PREFIX "lib"
    OUTPUT_NAME "diyardesk_plasma_gl"
)

target_link_libraries(diyardesk_plasma_gl PRIVATE
    Qt6::Core
    Qt6::Gui
    Qt6::Qml
    Qt6::Quick
    Qt6::OpenGL
    ${CMAKE_DL_LIBS}
)

configure_file(
    ${CMAKE_CURRENT_SOURCE_DIR}/qmldir
    ${CMAKE_CURRENT_BINARY_DIR}/qmldir
    COPYONLY
)
