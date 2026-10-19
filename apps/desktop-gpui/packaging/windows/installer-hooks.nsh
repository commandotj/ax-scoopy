; Tauri includes this file right after `SetCompressor /SOLID lzma`, before any
; data is compressed. A 128 MB window lets LZMA match scoopy-cli.exe against
; Scoopy.exe, which share most of their code; the default 8 MB window cannot.
SetCompressorDictSize 128

; Scoopy 0.6.0 ran its GPUI app as scoopy-gpui.exe next to Scoopy.exe. The template
; only closes Scoopy.exe before copying files, so a running scoopy-gpui.exe would keep
; the shared FFmpeg DLLs locked and stop the update with a file-in-use error.
!macro NSIS_HOOK_PREINSTALL
  !insertmacro CheckIfAppIsRunning "scoopy-gpui.exe" "${PRODUCTNAME}"
!macroend

!macro NSIS_HOOK_POSTINSTALL
  Delete "$INSTDIR\scoopy-gpui.exe"
  Delete "$INSTDIR\scoopy-exporter.exe"
  RMDir /r "$INSTDIR\assets\rive"
!macroend
