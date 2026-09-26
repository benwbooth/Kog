struct BSMemorySlot {
  Node::Port port;
  BSMemoryCartridge cartridge;

  //slot.cpp
  BSMemorySlot(string name);
  auto load(Node::Object) -> void;
  auto unload() -> void;

  const string name;
};

extern thread_local BSMemorySlot bsmemorySlot;
