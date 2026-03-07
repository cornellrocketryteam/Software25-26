{
  pkgsCross,
  stdenv,
  buildPackages,
  fetchFromGitHub,
}:
let
  pkgsCross32 = pkgsCross.armv7l-hf-multiplatform;
in
stdenv.mkDerivation (finalAttrs: {
  pname = "optee";
  version = "4.8.0";

  src = fetchFromGitHub {
    owner = "OP-TEE";
    repo = "optee_os";
    rev = finalAttrs.version;
    hash = "sha256-eefwfjSkDMFubKk+tIzTqe7h+v3VYxV6gzpzFxuJsyU=";
  };

  postPatch = ''
    patchShebangs scripts/ ta/pkcs11/scripts/
  '';

  strictDeps = true;
  enableParallelBuilding = true;

  depsBuildBuild = [ pkgsCross32.stdenv.cc ];
  nativeBuildInputs = [
    (buildPackages.python3.withPackages (p: [
      p.cryptography
      p.pyelftools
    ]))
  ];

  makeFlags = [
    "CROSS_COMPILE=${pkgsCross32.stdenv.cc.targetPrefix}"
    "CROSS_COMPILE64=${stdenv.cc.targetPrefix}"
    "CFG_ARM64_core=y"
    "PLATFORM=k3-am64x"
  ];

  installPhase = ''
    runHook preInstall

    mkdir -p $out
    cp out/arm-plat-k3/core/tee-raw.bin $out/

    runHook postInstall
  '';
})
