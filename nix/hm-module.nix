# Home-manager module for SuperFlow speech-to-text
#
# Provides a systemd user service for autostart.
# Usage: imports = [ superflow.homeManagerModules.default ];
#        services.superflow.enable = true;
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.services.superflow;
in
{
  options.services.superflow = {
    enable = lib.mkEnableOption "SuperFlow speech-to-text user service";

    package = lib.mkOption {
      type = lib.types.package;
      defaultText = lib.literalExpression "superflow.packages.\${system}.superflow";
      description = "The SuperFlow package to use.";
    };
  };

  config = lib.mkIf cfg.enable {
    systemd.user.services.superflow = {
      Unit = {
        Description = "SuperFlow speech-to-text";
        After = [ "graphical-session.target" ];
        PartOf = [ "graphical-session.target" ];
      };
      Service = {
        ExecStart = "${cfg.package}/bin/superflow";
        Restart = "on-failure";
        RestartSec = 5;
      };
      Install.WantedBy = [ "graphical-session.target" ];
    };
  };
}
