# ONNX Runtime with ROCm (MIGraphX and ROCm execution providers), for Kog's
# sound model on AMD GPUs. Only ONNX Runtime itself is compiled; ROCm comes
# from the binary cache. GPU targets default to RDNA3 (RX 7000 series);
# pass others with --argstr gpuTargets "gfx1030;gfx1100".
#
#   scripts/onnxruntime-rocm.sh   builds it and links it where Kog looks
{ pkgs ? (builtins.getFlake "git+file://${toString ../.}").inputs.nixpkgs.legacyPackages.${builtins.currentSystem}
, gpuTargets ? "gfx1100"
}:
(pkgs.onnxruntime.override {
  rocmSupport = true;
  cudaSupport = false;
  pythonSupport = false;
}).overrideAttrs (old: {
  pname = "onnxruntime-rocm";
  cmakeFlags = (old.cmakeFlags or [ ]) ++ [
    "-DCMAKE_HIP_ARCHITECTURES=${gpuTargets}"
    "-Donnxruntime_BUILD_UNIT_TESTS=OFF"
  ];
  doCheck = false;
})
