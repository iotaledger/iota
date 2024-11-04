{
  inputs = {
    nixpkgs-unstable.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    crate2nix.url = "github:nix-community/crate2nix";
  };

  outputs = { self, nixpkgs-unstable, crate2nix, flake-utils, ... }: 
    let
      git_revision = self.rev;
      iota_crates_to_fix = [
        "iota"
        "iota-framework-snapshot"
        "iota-bridge"
        "iota-graphql-rpc"
        "iota-move"
        "iota-move-lsp"
        "iota-node"
        "iota-source-validation-service"
        "iota-tools"
      ];
      crateNameToAttrSet = crate: { name = crate; value = attr: { CARGO_BIN_NAME = crate; GIT_REVISION = git_revision; };};
      iotaCratesOverrides = builtins.listToAttrs (builtins.map crateNameToAttrSet iota_crates_to_fix);
    in
    flake-utils.lib.eachDefaultSystem ( system:
      let
        pkgs = nixpkgs-unstable.legacyPackages.${system};
        customBuildRustCrateForPkgs = pkgs: pkgs.buildRustCrate.override {
          defaultCrateOverrides = pkgs.defaultCrateOverrides // iotaCratesOverrides //
          {
            librocksdb-sys = attrs: {
              buildInputs = with pkgs; [ clang zlib lz4 zstd ];
              nativeBuildInputs = [ pkgs.pkg-config ];
              LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
              extraLinkFlags = [
                "-L${pkgs.zlib.out}/lib"
                "-L${pkgs.lz4.lib}/lib"
                "-L${pkgs.zstd.out}/lib"
              ];
            };
          };
        };
        generateBuild = pkgs.callPackage ./Cargo.nix {
          buildRustCrateForPkgs = customBuildRustCrateForPkgs;
        };
      in {
        packages = rec {
          iota = generateBuild.workspaceMembers."iota".build;
          default = iota;
        };
      }
    );
}