"""Generate Song Player's code-native application icon (requires Pillow)."""
from pathlib import Path
from PIL import Image, ImageDraw

root = Path(__file__).resolve().parent.parent
out = root / "src-tauri" / "icons"
out.mkdir(parents=True, exist_ok=True)
im = Image.new("RGBA", (512, 512))
draw = ImageDraw.Draw(im)
draw.rounded_rectangle((16, 16, 496, 496), radius=115, fill="#19332d")
for x, height in [(140, 100), (198, 190), (256, 280), (314, 190), (372, 100)]:
    draw.rounded_rectangle((x - 17, 256 - height // 2, x + 17, 256 + height // 2), radius=17, fill="#75dfb4")
im.save(out / "icon.png")
im.save(out / "icon.ico", sizes=[(16, 16), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
