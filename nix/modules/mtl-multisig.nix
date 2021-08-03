{ config, lib, pkgs, ... }:
with lib;  # use the functions from lib, such as mkIf
let
  # the values of the options set for the service by the user of the service
  cfg = config.services.mtl-multisig;
in {
  ##### interface. here we define the options that users of our service can specify
  options = {
    # the options for our service will be located under services.mtl-multisig
    services.mtl-multisig = {
      enable = mkOption {
        type = types.bool;
        default = false;
        description = ''
          Whether to enable Montelibero multisignature service by default.
        '';
      };
      package = mkOption {
        type = types.package;
        default = pkgs.mtl-multisig;
        description = ''
          Which package to use with the service.
        '';
      };
      port = mkOption {
        type = types.int;
        default = 9857;
        description = ''
          Which port the indexer listen to connections.
        '';
      };
      statePath = mkOption {
        type = types.str;
        default = "/var/lib/mtl-multisig";
        description = ''
          Path to database on filesystem.
        '';
      };
      statics = mkOption {
        type = types.str;
        default = "${cfg.package}/share/static";
        description = ''
          Path to static files of webserver.
        '';
      };
      templates = mkOption {
        type = types.str;
        default = "${cfg.package}/share/templates";
        description = ''
          Path to templates files of webserver.
        '';
      };
      config = mkOption {
        type = types.str;
        default = ''
          [default]
          port=${toString cfg.port}
          statics="${cfg.statics}"
          template_dir="${cfg.templates}"

          [global.databases]
          transactions = { url = "${cfg.statePath}/database.sqlite" }
        '';
        description = ''
          Configuration file.
        '';
      };
    };
  };

  ##### implementation
  config = mkIf cfg.enable { # only apply the following settings if enabled
    nixpkgs.overlays = [
      (import ../overlay.nix)
    ];
    environment.etc."mtl-multisig.toml" = {
      text = cfg.config; # we can use values of options for this service here
    };
    # Create systemd service
    systemd.services.mtl-multisig = {
      enable = true;
      description = "Ergvein indexer node";
      after = ["network.target"];
      wants = ["network.target"];
      script = ''
        ROCKET_CONFIG=/etc/mtl-multisig.toml ${cfg.package}/bin/multisig-service
      '';
      serviceConfig = {
          Restart = "always";
          RestartSec = 30;
          User = "root";
          LimitNOFILE = 65536;
        };
      wantedBy = ["multi-user.target"];
    };
    # Init folder for bitcoin data
    system.activationScripts = {
      int-mtl-multisig = {
        text = ''
          if [ ! -d "${cfg.statePath}" ]; then
            mkdir -p ${cfg.statePath}
          fi
        '';
        deps = [];
      };
    };
  };
}
