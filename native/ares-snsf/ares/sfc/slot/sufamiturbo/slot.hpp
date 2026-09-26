struct SufamiTurboSlot {
  Node::Port port;
  SufamiTurboCartridge cartridge;

  //slot.cpp
  SufamiTurboSlot(string name);
  auto load(Node::Object) -> void;
  auto unload() -> void;

  const string name;
};

extern thread_local SufamiTurboSlot sufamiturboSlotA;
extern thread_local SufamiTurboSlot sufamiturboSlotB;
