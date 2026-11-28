mode = None
if iframe:
    mode = "kiosk"
else:
    mode = "mobile" if mobile else "desktop"
if mode is None and phone:
    mode = "mobile"
if mode is None and tablet:
    mode = "tablet"
if layout is not None and wide:
    layout = "wide"
