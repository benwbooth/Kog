# Adapt the pinned upstream at configure time; keep the submodule immutable.
# Exact replacement guards deliberately stop a future upstream upgrade for review.
function(kog_replace_checked variable before after)
    string(FIND "${${variable}}" "${before}" position)
    if(position LESS 0)
        message(FATAL_ERROR "Pinned Play! adaptation no longer matches: ${before}")
    endif()
    string(REPLACE "${before}" "${after}" updated "${${variable}}")
    set(${variable} "${updated}" PARENT_SCOPE)
endfunction()

file(READ "${PLAY_SOURCE}/Source/iop/Iop_SubSystem.cpp" iop_source)
kog_replace_checked(iop_source "#include \"GenericMipsExecutor.h\"" "#include \"iop_interpreter.h\"")
kog_replace_checked(iop_source
    "std::make_unique<CGenericMipsExecutor<BlockLookupOneWay>>(m_cpu, (IOP_RAM_SIZE * 4), BLOCK_CATEGORY_PS2_IOP)"
    "std::make_unique<KogIopInterpreter>(m_cpu, ps2Mode)")
kog_replace_checked(iop_source "if(m_intc.HasPendingInterrupt())"
    "if(m_intc.HasPendingInterrupt() && m_cpu.m_State.nDelayedJumpAddr == MIPS_INVALID_PC)")
kog_replace_checked(iop_source "m_bios->HandleInterrupt();"
    "if(m_cpu.CanGenerateInterrupt()) static_cast<KogIopInterpreter*>(m_cpu.m_executor.get())->PrepareInterrupt();\n\t\t\tm_bios->HandleInterrupt();")
file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/Kog_Iop_SubSystem.cpp" "${iop_source}")

# The original generated HLE handlers assume immediate load results, as does
# Play!'s JIT. Schedule their loads for the R3000's real one-instruction delay.
foreach(bios IN ITEMS "iop/IopBios" "psx/PsxBios")
    file(READ "${PLAY_SOURCE}/Source/${bios}.cpp" bios_source)
    if(bios STREQUAL "iop/IopBios")
        # A load in a call's delay slot cannot be followed by a local NOP: move
        # it before the call so the callee receives its argument immediately.
        foreach(pair IN ITEMS "V0;0x04;S0" "T1;offsetof(VBLANKHANDLER, arg);T0")
            list(GET pair 0 target)
            list(GET pair 1 offset)
            list(GET pair 2 base)
            kog_replace_checked(bios_source
                "assembler.JALR(CMIPS::${target});\n\tassembler.LW(CMIPS::A0, ${offset}, CMIPS::${base});"
                "assembler.LW(CMIPS::A0, ${offset}, CMIPS::${base});\n\tassembler.NOP();\n\tassembler.JALR(CMIPS::${target});\n\tassembler.NOP();")
        endforeach()
    endif()
    string(REGEX REPLACE "(assembler\\.LW\\([^;]*;)(\n)" "\\1\n\tassembler.NOP();\\2" bios_source "${bios_source}")
    get_filename_component(name "${bios}" NAME)
    file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/Kog_${name}.cpp" "${bios_source}")
endforeach()

get_target_property(play_sources PlayCore SOURCES)
list(REMOVE_ITEM play_sources iop/Iop_SubSystem.cpp iop/IopBios.cpp psx/PsxBios.cpp)
set_property(TARGET PlayCore PROPERTY SOURCES "${play_sources}")
target_sources(PlayCore PRIVATE
    "${CMAKE_CURRENT_BINARY_DIR}/Kog_Iop_SubSystem.cpp"
    "${CMAKE_CURRENT_BINARY_DIR}/Kog_IopBios.cpp"
    "${CMAKE_CURRENT_BINARY_DIR}/Kog_PsxBios.cpp"
    "${CMAKE_CURRENT_SOURCE_DIR}/iop_interpreter.cpp")
target_include_directories(PlayCore PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}"
    "${PLAY_SOURCE}/Source/iop" "${PLAY_SOURCE}/Source/psx")

# Expose frame timing without changing the vendored API in place. PSF1 captures
# can explicitly request 50 Hz, independently of the fixed 44.1 kHz output rate.
set(psf_source "${PLAY_SOURCE}/tools/PsfPlayer/Source")
file(READ "${psf_source}/Iop_PsfSubSystem.h" psf_header)
kog_replace_checked(psf_header "void Reset() override;"
    "void SetFrameRate(uint32 cpuFrequency, uint32 frameRate) { m_frameTicks = cpuFrequency / frameRate; m_frameCounter = m_frameTicks; }\n\t\tvoid Reset() override;")
file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/Kog_PsfSubSystem.h" "${psf_header}")
file(READ "${psf_source}/Iop_PsfSubSystem.cpp" psf_implementation)
kog_replace_checked(psf_implementation "#include \"Iop_PsfSubSystem.h\"" "#include \"Kog_PsfSubSystem.h\"")
file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/Kog_PsfSubSystem.cpp" "${psf_implementation}")
