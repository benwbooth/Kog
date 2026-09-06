/**
 * dispatch_codes.m
 *
 * dispatch message definitions
 *
 * @author mpdeimos
 * @date 2008/10/25
 * @version 1
 */

#ifndef included
#error This script can only be compiled as a #include
#endif

#include <lib/com/dispatch_ifc.m>

#define SUCCESS 1
#define FAIL 0

/*
 *  Messages
 */

#define ON_LEFT_BUTTON_DOWN 0001
#define ON_MOUSE_MOVE 0002
#define ON_LEFT_BUTTON_UP 0003
#define ON_TAB_TEXT_UPDATED 0004
#define ON_TAB_ACTIVATED 0005
#define ON_TAB_OFF 1111
#define ON_RIGHT_BUTTON_UP 0006
#define ON_ENTER_AREA 0007
#define ON_LEAVE_AREA 0008
#define ON_LEFT_DBL_CLICK 2222