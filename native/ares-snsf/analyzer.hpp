// Generated from pinned ISC ares/mia/medium/super-famicom.cpp by vendor.py.
struct SnsfCartridgeAnalysis {
  std::vector<u8> rom;
  u32 headerAddress = 0;
  auto size() const -> u32 { return rom.size(); }
  auto region() const -> string;
  auto videoRegion() const -> string;
  auto board() const -> string;
  auto label() const -> string;
  auto serial() const -> string;
  auto romSize() const -> u32;
  auto programRomSize() const -> u32;
  auto dataRomSize() const -> u32;
  auto expansionRomSize() const -> u32;
  auto firmwareRomSize() const -> u32;
  auto ramSize() const -> u32;
  auto expansionRamSize() const -> u32;
  auto scoreHeader(u32 address) -> u32;
};
auto SnsfCartridgeAnalysis::region() const -> string {
  //Unlicensed software (homebrew, ROM hacks, etc) often change the standard region code,
  //and then neglect to change the extended header region code. Thanks to that, we can't
  //decode and display the full game serial + region code.
  return videoRegion();

  string region;

  char A = rom[headerAddress + 0x02];  //game type
  char B = rom[headerAddress + 0x03];  //game code
  char C = rom[headerAddress + 0x04];  //game code
  char D = rom[headerAddress + 0x05];  //region code (new; sometimes ambiguous)
  auto E = rom[headerAddress + 0x29];  //region code (old)

  auto valid = [](char n) { return (n >= '0' && n <= '9') || (n >= 'A' && n <= 'Z'); };
  if(rom[headerAddress + 0x2a] == 0x33 && valid(A) && valid(B) & valid(C) & valid(D)) {
    string code{A, B, C, D};
    if(D == 'B') region = {"SNS-",  code, "-BRA"};
    if(D == 'C') region = {"SNSN-", code, "-ROC"};
    if(D == 'D') region = {"SNSP-", code, "-NOE"};
    if(D == 'E') region = {"SNS-",  code, "-USA"};
    if(D == 'F') region = {"SNSP-", code, "-FRA"};
    if(D == 'H') region = {"SNSP-", code, "-HOL"};
    if(D == 'I') region = {"SNSP-", code, "-ITA"};
    if(D == 'J') region = {"SHVC-", code, "-JPN"};
    if(D == 'K') region = {"SNSN-", code, "-KOR"};
    if(D == 'N') region = {"SNS-",  code, "-CAN"};
    if(D == 'P') region = {"SNSP-", code, "-EUR"};
    if(D == 'S') region = {"SNSP-", code, "-ESP"};
    if(D == 'U') region = {"SNSP-", code, "-AUS"};
    if(D == 'W') region = {"SNSP-", code, "-SCN"};
  }

  if(!region) {
    if(E == 0x00) region = {"JPN"};
    if(E == 0x01) region = {"USA"};
    if(E == 0x02) region = {"EUR"};
    if(E == 0x03) region = {"SCN"};
    if(E == 0x06) region = {"FRA"};
    if(E == 0x07) region = {"HOL"};
    if(E == 0x08) region = {"ESP"};
    if(E == 0x09) region = {"NOE"};
    if(E == 0x0a) region = {"ITA"};
    if(E == 0x0b) region = {"ROC"};
    if(E == 0x0d) region = {"KOR"};
    if(E == 0x0f) region = {"CAN"};
    if(E == 0x10) region = {"BRA"};
    if(E == 0x11) region = {"AUS"};
  }

  return region ? region : "NTSC"_s;
}

auto SnsfCartridgeAnalysis::videoRegion() const -> string {
  auto region = rom[headerAddress + 0x29];
  if(region == 0x00) return "NTSC";  //JPN
  if(region == 0x01) return "NTSC";  //USA
  if(region == 0x0b) return "NTSC";  //ROC
  if(region == 0x0d) return "NTSC";  //KOR
  if(region == 0x0f) return "NTSC";  //CAN
  if(region == 0x10) return "NTSC";  //BRA
  return "PAL";
}

auto SnsfCartridgeAnalysis::board() const -> string {
  string board;

  auto mapMode          = rom[headerAddress + 0x25];
  auto cartridgeTypeLo  = rom[headerAddress + 0x26] & 15;
  auto cartridgeTypeHi  = rom[headerAddress + 0x26] >> 4;
  auto cartridgeSubType = rom[headerAddress + 0x0f];

  string mode;
  if(mapMode == 0x20 || mapMode == 0x30) mode = "LOROM-";
  if(mapMode == 0x21 || mapMode == 0x31) mode = "HIROM-";
  if(mapMode == 0x22 || mapMode == 0x32) mode = "SDD1-";
  if(mapMode == 0x23 || mapMode == 0x33) mode = "SA1-";
  if(mapMode == 0x25 || mapMode == 0x35) mode = "EXHIROM-";
  if(mapMode == 0x2a || mapMode == 0x3a) mode = "SPC7110-";

  //many games will store an extra title character, overwriting the map mode
  //further, ExLoROM mode is unofficial, and lacks a mapping mode value
  if(!mode) {
    if(headerAddress ==   0x7fb0) mode = "LOROM-";
    if(headerAddress ==   0xffb0) mode = "HIROM-";
    if(headerAddress == 0x407fb0) mode = "EXLOROM-";
    if(headerAddress == 0x40ffb0) mode = "EXHIROM-";
  }

  //the Sufami Turbo has the non-descriptive label "ADD-ON BASE CASSETE"
  //(yes, missing a T), and its serial "A9PJ" is shared with
  //Bishoujo Senshi Sailor Moon SuperS - Fuwafuwa Panic (Japan)
  //so we identify it with this embedded string
  string sufamiSignature = "BANDAI SFC-ADX";
  auto romSignature = string_view((const char*)rom.data(), sufamiSignature.length());
  if(romSignature == sufamiSignature) board.append("ST-", mode);

  //this game's title overwrite the map mode with '!' (0x21), but is a LOROM game
  if(label() == "YUYU NO QUIZ DE GO!GO") mode = "LOROM-";

  //some ExLoROM titles have mode = LOROM-, let's fix that
  if(mode == "LOROM-" && headerAddress == 0x407fb0) mode = "EXLOROM-";

  bool epsonRTC = false;
  bool sharpRTC = false;

  if(serial() == "ZBSJ") {
  //BS-X: Sore wa Namae o Nusumareta Machi no Monogatari (JPN)
    board.append("BS-MCC-");
  } else if(serial() == "042J") {
  //Super Game Boy 2
    board.append("GB-", mode);
  } else if(serial().match("Z??J")) {
    board.append("BS-", mode);
  } else if(cartridgeTypeLo >= 0x3) {
    if(cartridgeTypeHi == 0x0) board.append("NEC-", mode);
    if(cartridgeTypeHi == 0x1) board.append("GSU-");
    if(cartridgeTypeHi == 0x2) board.append("OBC1-", mode);
    if(cartridgeTypeHi == 0x3) board.append("SA1-");
    if(cartridgeTypeHi == 0x4) board.append("SDD1-");
    if(cartridgeTypeHi == 0x5) board.append(mode), sharpRTC = true;
    if(cartridgeTypeHi == 0xe && cartridgeTypeLo == 0x3) board.append("GB-", mode);
    if(cartridgeTypeHi == 0xf && cartridgeTypeLo == 0x5 && cartridgeSubType == 0x00) board.append("SPC7110-");
    if(cartridgeTypeHi == 0xf && cartridgeTypeLo == 0x9 && cartridgeSubType == 0x00) board.append("SPC7110-"), epsonRTC = true;
    if(cartridgeTypeHi == 0xf                           && cartridgeSubType == 0x01) board.append("EXNEC-", mode);
    if(cartridgeTypeHi == 0xf                           && cartridgeSubType == 0x02) board.append("ARM-", mode);
    if(cartridgeTypeHi == 0xf                           && cartridgeSubType == 0x10) board.append("HITACHI-", mode);
  }
  if(!board) board.append(mode);

  if(ramSize() || expansionRamSize()) board.append("RAM-");
  if(epsonRTC) board.append("EPSONRTC-");
  if(sharpRTC) board.append("SHARPRTC-");

  board.trimRight("-", 1L);

  if(board.beginsWith(    "LOROM-RAM") && romSize() <= 0x200000) board.append("#A");
  if(board.beginsWith("NEC-LOROM-RAM") && romSize() <= 0x100000) board.append("#A");

  //Tengai Makyou Zero (fan translation)
  if(board.beginsWith("SPC7110-") && romSize() == 0x700000) board.prepend("EX");

  return board;
}

auto SnsfCartridgeAnalysis::label() const -> string {
  string label;

  for(u32 n = 0; n < 0x15; n++) {
    auto x = rom[headerAddress + 0x10 + n];
    auto y = n == 0x14 ? 0 : rom[headerAddress + 0x11 + n];

    //null terminator (padding)
    if(x == 0x00 || x == 0xff);

    //ASCII
    else if(x >= 0x20 && x <= 0x7e) label.append((char)x);

    //Shift-JIS (half-width katakana)
    else if(x == 0xa1) label.append("。");
    else if(x == 0xa2) label.append("「");
    else if(x == 0xa3) label.append("」");
    else if(x == 0xa4) label.append("、");
    else if(x == 0xa5) label.append("・");
    else if(x == 0xa6) label.append("ヲ");
    else if(x == 0xa7) label.append("ァ");
    else if(x == 0xa8) label.append("ィ");
    else if(x == 0xa9) label.append("ゥ");
    else if(x == 0xaa) label.append("ェ");
    else if(x == 0xab) label.append("ォ");
    else if(x == 0xac) label.append("ャ");
    else if(x == 0xad) label.append("ュ");
    else if(x == 0xae) label.append("ョ");
    else if(x == 0xaf) label.append("ッ");
    else if(x == 0xb0) label.append("ー");

    else if(x == 0xb1) label.append(                 "ア");
    else if(x == 0xb2) label.append(                 "イ");
    else if(x == 0xb3) label.append(y == 0xde ? "ヴ" : "ウ");
    else if(x == 0xb4) label.append(                 "エ");
    else if(x == 0xb5) label.append(                 "オ");

    else if(x == 0xb6) label.append(y == 0xde ? "ガ" : "カ");
    else if(x == 0xb7) label.append(y == 0xde ? "ギ" : "キ");
    else if(x == 0xb8) label.append(y == 0xde ? "グ" : "ク");
    else if(x == 0xb9) label.append(y == 0xde ? "ゲ" : "ケ");
    else if(x == 0xba) label.append(y == 0xde ? "ゴ" : "コ");

    else if(x == 0xbb) label.append(y == 0xde ? "ザ" : "サ");
    else if(x == 0xbc) label.append(y == 0xde ? "ジ" : "シ");
    else if(x == 0xbd) label.append(y == 0xde ? "ズ" : "ス");
    else if(x == 0xbe) label.append(y == 0xde ? "ゼ" : "セ");
    else if(x == 0xbf) label.append(y == 0xde ? "ゾ" : "ソ");

    else if(x == 0xc0) label.append(y == 0xde ? "ダ" : "タ");
    else if(x == 0xc1) label.append(y == 0xde ? "ヂ" : "チ");
    else if(x == 0xc2) label.append(y == 0xde ? "ヅ" : "ツ");
    else if(x == 0xc3) label.append(y == 0xde ? "デ" : "テ");
    else if(x == 0xc4) label.append(y == 0xde ? "ド" : "ト");

    else if(x == 0xc5) label.append("ナ");
    else if(x == 0xc6) label.append("ニ");
    else if(x == 0xc7) label.append("ヌ");
    else if(x == 0xc8) label.append("ネ");
    else if(x == 0xc9) label.append("ノ");

    else if(x == 0xca) label.append(y == 0xdf ? "パ" : y == 0xde ? "バ" : "ハ");
    else if(x == 0xcb) label.append(y == 0xdf ? "ピ" : y == 0xde ? "ビ" : "ヒ");
    else if(x == 0xcc) label.append(y == 0xdf ? "プ" : y == 0xde ? "ブ" : "フ");
    else if(x == 0xcd) label.append(y == 0xdf ? "ペ" : y == 0xde ? "ベ" : "ヘ");
    else if(x == 0xce) label.append(y == 0xdf ? "ポ" : y == 0xde ? "ボ" : "ホ");

    else if(x == 0xcf) label.append("マ");
    else if(x == 0xd0) label.append("ミ");
    else if(x == 0xd1) label.append("ム");
    else if(x == 0xd2) label.append("メ");
    else if(x == 0xd3) label.append("モ");

    else if(x == 0xd4) label.append("ヤ");
    else if(x == 0xd5) label.append("ユ");
    else if(x == 0xd6) label.append("ヨ");

    else if(x == 0xd7) label.append("ラ");
    else if(x == 0xd8) label.append("リ");
    else if(x == 0xd9) label.append("ル");
    else if(x == 0xda) label.append("レ");
    else if(x == 0xdb) label.append("ロ");

    else if(x == 0xdc) label.append("ワ");
    else if(x == 0xdd) label.append("ン");

    else if(x == 0xde) label.append("\xef\xbe\x9e");  //dakuten
    else if(x == 0xdf) label.append("\xef\xbe\x9f");  //handakuten

    //unknown
    else label.append("?");

    //(han)dakuten skip
    if(y == 0xde && x == 0xb3) n++;
    if(y == 0xde && x >= 0xb6 && x <= 0xc4) n++;
    if(y == 0xde && x >= 0xca && x <= 0xce) n++;
    if(y == 0xdf && x >= 0xca && y <= 0xce) n++;
  }

  return label.strip();
}

auto SnsfCartridgeAnalysis::serial() const -> string {
  char A = rom[headerAddress + 0x02];  //game type
  char B = rom[headerAddress + 0x03];  //game code
  char C = rom[headerAddress + 0x04];  //game code
  char D = rom[headerAddress + 0x05];  //region code (new; sometimes ambiguous)

  auto valid = [](char n) { return (n >= '0' && n <= '9') || (n >= 'A' && n <= 'Z'); };
  if(rom[headerAddress + 0x2a] == 0x33 && valid(A) && valid(B) & valid(C) & valid(D)) {
    return {A, B, C, D};
  }

  return "";
}

auto SnsfCartridgeAnalysis::romSize() const -> u32 {
  return size() - firmwareRomSize();
}

auto SnsfCartridgeAnalysis::programRomSize() const -> u32 {
  if(board().beginsWith("SPC7110-")) return 0x100000;
  if(board().beginsWith("EXSPC7110-")) return 0x100000;
  return romSize();
}

auto SnsfCartridgeAnalysis::dataRomSize() const -> u32 {
  if(board().beginsWith("SPC7110-")) return romSize() - 0x100000;
  if(board().beginsWith("EXSPC7110-")) return 0x500000;
  return 0;
}

auto SnsfCartridgeAnalysis::expansionRomSize() const -> u32 {
  if(board().beginsWith("EXSPC7110-")) return 0x100000;
  return 0;
}

auto SnsfCartridgeAnalysis::firmwareRomSize() const -> u32 {
  auto cartridgeTypeLo  = rom[headerAddress + 0x26] & 15;
  auto cartridgeTypeHi  = rom[headerAddress + 0x26] >> 4;
  auto cartridgeSubType = rom[headerAddress + 0x0f];

  if(serial() == "042J" || (cartridgeTypeLo == 0x3 && cartridgeTypeHi == 0xe)) {
    //Game Boy
    if((size() & 0x7fff) == 0x100) return 0x100;
  }

  if(cartridgeTypeLo >= 0x3 && cartridgeTypeHi == 0xf && cartridgeSubType == 0x10) {
    //Hitachi HG51BS169
    if((size() & 0x7fff) == 0xc00) return 0xc00;
  }

  if(cartridgeTypeLo >= 0x3 && cartridgeTypeHi == 0x0) {
    //NEC uPD7725
    if((size() & 0x7fff) == 0x2000) return 0x2000;
  }

  if(cartridgeTypeLo >= 0x3 && cartridgeTypeHi == 0xf && cartridgeSubType == 0x01) {
    //NEC uPD96050
    if((size() & 0xffff) == 0xd000) return 0xd000;
  }

  if(cartridgeTypeLo >= 0x3 && cartridgeTypeHi == 0xf && cartridgeSubType == 0x02) {
    //ARM6
    if((size() & 0x3ffff) == 0x28000) return 0x28000;
  }

  return 0;
}

auto SnsfCartridgeAnalysis::ramSize() const -> u32 {
  auto ramSize = rom[headerAddress + 0x28];
  if (ramSize) return (1 << (((ramSize - 1) & 7) + 1)) << 10;
  return 0;
}

auto SnsfCartridgeAnalysis::expansionRamSize() const -> u32 {
  if(rom[headerAddress + 0x2a] == 0x33) {
    auto ramSize = rom[headerAddress + 0x0d] & 7;
    if(ramSize) return 1024 << ramSize;
  }
  if((rom[headerAddress + 0x26] >> 4) == 1) {
    //GSU: Starfox / Starwing lacks an extended header; but still has expansion RAM
    return 0x8000;
  }
  return 0;
}

auto SnsfCartridgeAnalysis::scoreHeader(u32 address) -> u32 {
  s32 score = 0;
  if(size() < address + 0x50) return score;

  u8  mapMode     = rom[address + 0x25] & ~0x10;  //ignore FastROM bit
  u16 complement  = rom[address + 0x2c] << 0 | rom[address + 0x2d] << 8;
  u16 checksum    = rom[address + 0x2e] << 0 | rom[address + 0x2f] << 8;
  u16 resetVector = rom[address + 0x4c] << 0 | rom[address + 0x4d] << 8;
  if(resetVector < 0x8000) return score;  //$00:0000-7fff is never ROM data

  u8 opcode = rom[(address & ~0x7fff) | (resetVector & 0x7fff)];  //first instruction executed

  //most likely opcodes
  if(opcode == 0x78  //sei
  || opcode == 0x18  //clc (clc; xce)
  || opcode == 0x38  //sec (sec; xce)
  || opcode == 0x9c  //stz $nnnn (stz $4200)
  || opcode == 0x4c  //jmp $nnnn
  || opcode == 0x5c  //jml $nnnnnn
  ) score += 8;

  //plausible opcodes
  if(opcode == 0xc2  //rep #$nn
  || opcode == 0xe2  //sep #$nn
  || opcode == 0xad  //lda $nnnn
  || opcode == 0xae  //ldx $nnnn
  || opcode == 0xac  //ldy $nnnn
  || opcode == 0xaf  //lda $nnnnnn
  || opcode == 0xa9  //lda #$nn
  || opcode == 0xa2  //ldx #$nn
  || opcode == 0xa0  //ldy #$nn
  || opcode == 0x20  //jsr $nnnn
  || opcode == 0x22  //jsl $nnnnnn
  ) score += 4;

  //implausible opcodes
  if(opcode == 0x40  //rti
  || opcode == 0x60  //rts
  || opcode == 0x6b  //rtl
  || opcode == 0xcd  //cmp $nnnn
  || opcode == 0xec  //cpx $nnnn
  || opcode == 0xcc  //cpy $nnnn
  ) score -= 4;

  //least likely opcodes
  if(opcode == 0x00  //brk #$nn
  || opcode == 0x02  //cop #$nn
  || opcode == 0xdb  //stp
  || opcode == 0x42  //wdm
  || opcode == 0xff  //sbc $nnnnnn,x
  ) score -= 8;

  if(checksum + complement == 0xffff) score += 4;

  if(address == 0x7fb0 && mapMode == 0x20) score += 2;
  if(address == 0xffb0 && mapMode == 0x21) score += 2;

  return max(0, score);
}

