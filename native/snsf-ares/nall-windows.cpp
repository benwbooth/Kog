// SPDX-License-Identifier: ISC
// Windows needs nall's out-of-line platform helpers. Compile the pinned
// upstream implementations using their original unity include order.
#include <nall/windows/windows.hpp>
#include <nall/dl.cpp>
#include <nall/platform.cpp>
#include <nall/terminal.cpp>
#include <nall/thread.cpp>
#include <nall/windows/utf8.cpp>
