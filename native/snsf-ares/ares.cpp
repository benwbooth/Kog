// SPDX-License-Identifier: ISC
// Headless configuration for the ares core; upstream implementation is in ares-snsf.
#include <ares/ares.hpp>
#include <ares/debug/debug.cpp>
#include <ares/node/node.cpp>
namespace ares {
thread_local Platform* platform = nullptr;
thread_local atomic<bool> _runAhead = false;
const string Name = "ares (Kog SNSF)";
const string Version = "4cb8d92b441557cb6bcaf133c4cbc7f6819b1122";
const string Copyright = "Copyright (c) 2004-2025 ares team, Near et al";
const string License = "ISC";
const string LicenseURI = "https://opensource.org/licenses/ISC";
const string Website = "ares-emu.net";
const string WebsiteURI = "https://ares-emu.net/";
const u32 SerializerSignature = 0x31545342;
}
