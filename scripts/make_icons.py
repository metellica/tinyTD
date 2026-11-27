# 生成 tinyTD 应用图标（圆角渐变底 + 白色对勾）
# 用法: python scripts/make_icons.py
import os

from PIL import Image, ImageDraw

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "src-tauri", "icons")
os.makedirs(OUT, exist_ok=True)

S = 512
TOP, BOT = (99, 102, 241), (139, 92, 246)  # 靛蓝 -> 紫

# 垂直渐变
grad = Image.new("RGBA", (S, S))
d = ImageDraw.Draw(grad)
for y in range(S):
    t = y / (S - 1)
    c = tuple(int(TOP[i] + (BOT[i] - TOP[i]) * t) for i in range(3)) + (255,)
    d.line([(0, y), (S, y)], fill=c)

# 圆角蒙版
mask = Image.new("L", (S, S), 0)
ImageDraw.Draw(mask).rounded_rectangle([6, 6, S - 6, S - 6], radius=116, fill=255)

icon = Image.new("RGBA", (S, S), (0, 0, 0, 0))
icon.paste(grad, (0, 0), mask)

# 白色对勾（圆头线帽）
d = ImageDraw.Draw(icon)
pts = [(150, 268), (233, 350), (366, 208)]
d.line(pts, fill=(255, 255, 255, 255), width=44, joint="curve")
r = 22
for (x, y) in (pts[0], pts[-1]):
    d.ellipse([x - r, y - r, x + r, y + r], fill=(255, 255, 255, 255))

icon.save(os.path.join(OUT, "icon.png"))
icon.resize((256, 256), Image.LANCZOS).save(os.path.join(OUT, "128x128@2x.png"))
icon.resize((128, 128), Image.LANCZOS).save(os.path.join(OUT, "128x128.png"))
icon.resize((32, 32), Image.LANCZOS).save(os.path.join(OUT, "32x32.png"))
icon.resize((256, 256), Image.LANCZOS).save(
    os.path.join(OUT, "icon.ico"),
    sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)],
)
print("icons written to", os.path.abspath(OUT))
