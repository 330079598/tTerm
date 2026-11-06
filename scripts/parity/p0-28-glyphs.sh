#!/bin/sh
# Prints the same 18 by 13 grid of built-in glyphs as
# `cargo run -p tterm-gpui --example glyph_grid`.
#
# In the Tauri build of tTerm (the xterm.js version), open a terminal at least
# 80 columns wide, run this script, and screenshot the output. Save the image
# as docs/gpui-migration/reports/assets/P0-28-tauri-glyphs-[manual].png and
# record it in the P0-28 report. That screenshot is a [manual] step: the GPUI
# side has docs/gpui-migration/reports/assets/P0-28-macos.png.
#
# The order matches the example: U+2500..U+257F box drawing, U+2580..U+259F
# blocks, U+1FB00..U+1FB3B sextants, U+1FB82..U+1FB8B legacy blocks, and
# U+E0B0..U+E0B3 Powerline.
set -eu

printf 'tTerm P0-28 built-in glyphs: 18 columns x 13 rows\n'
printf 'Compare with docs/gpui-migration/reports/assets/P0-28-macos.png\n\n'
printf '%s\n' \
    '─━│┃┄┅┆┇┈┉┊┋┌┍┎┏┐┑' \
    '┒┓└┕┖┗┘┙┚┛├┝┞┟┠┡┢┣' \
    '┤┥┦┧┨┩┪┫┬┭┮┯┰┱┲┳┴┵' \
    '┶┷┸┹┺┻┼┽┾┿╀╁╂╃╄╅╆╇' \
    '╈╉╊╋╌╍╎╏═║╒╓╔╕╖╗╘╙' \
    '╚╛╜╝╞╟╠╡╢╣╤╥╦╧╨╩╪╫' \
    '╬╭╮╯╰╱╲╳╴╵╶╷╸╹╺╻╼╽' \
    '╾╿▀▁▂▃▄▅▆▇█▉▊▋▌▍▎▏' \
    '▐░▒▓▔▕▖▗▘▙▚▛▜▝▞▟🬀🬁' \
    '🬂🬃🬄🬅🬆🬇🬈🬉🬊🬋🬌🬍🬎🬏🬐🬑🬒🬓' \
    '🬔🬕🬖🬗🬘🬙🬚🬛🬜🬝🬞🬟🬠🬡🬢🬣🬤🬥' \
    '🬦🬧🬨🬩🬪🬫🬬🬭🬮🬯🬰🬱🬲🬳🬴🬵🬶🬷' \
    '🬸🬹🬺🬻🮂🮃🮄🮅🮆🮇🮈🮉🮊🮋' \
    ''

printf '\nEnd of the P0-28 glyph grid.\n'
