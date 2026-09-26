//DIP switch
//used for Nintendo Super System emulation

thread_local DIP dip;
#include "serialization.cpp"

auto DIP::power() -> void {
}

auto DIP::read(n24, n8) -> n8 {
  return value;
}

auto DIP::write(n24, n8) -> void {
}
