-- Midnight Bump: fresh-install Hyprland Lua configuration (0.56+).
-- Keep desktop-specific display overrides outside this file.
hl.monitor({ output = "", mode = "preferred", position = "auto", scale = "auto" })

hl.config({
    general = {
        gaps_in = 4,
        gaps_out = 8,
        border_size = 2,
        layout = "dwindle",
        col = {
            active_border = "rgba(d6ac75ee)",
            inactive_border = "rgba(424a4eaa)",
        },
    },
    decoration = {
        rounding = 8,
        active_opacity = 1.0,
        inactive_opacity = 0.96,
        blur = { enabled = true, size = 4, passes = 2 },
        shadow = { enabled = true, range = 4, render_power = 3, color = 0xee14181c },
    },
    input = {
        kb_layout = "us",
        follow_mouse = 1,
        touchpad = { natural_scroll = false },
    },
    misc = {
        disable_hyprland_logo = true,
        force_default_wallpaper = -1,
    },
})

-- One systemd-managed Waybar/Mako/wallpaper stack. Do not start them here.
-- Applets are session programs and are launched exactly once per Hyprland start.
hl.on("hyprland.start", function()
    hl.exec_cmd("nm-applet --indicator")
    hl.exec_cmd("blueman-applet")
end)

require("keybindings")
