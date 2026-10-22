# Theme cua Office

Moi theme la mot thu muc trong `themes/`. Chon theme trong Settings (⚙).

## Tao theme moi

1. Copy `themes/default` thanh `themes/<ten-moi>`.
2. Sua `theme.json`:
   - `colors`: mau giao dien (`bg`, `room`, `border`, `text`, `muted`, `accent`, `review`, `waiting`, `running`, `idle`, `warn`, `bubble-text`, `error`, `shadow`, `input`).
   - `font`: font chu.
   - `sprite.frame`: kich thuoc mot khung anh (px). `sprite.scale`: phong to bao nhieu lan.
   - `sprite.characterY`, `sprite.deskY`: dich nhan vat / ban len xuong (px cua khung) cho dau nhan vat lo tren ban.
   - `slots`: ten slot -> file anh (SVG, PNG, GIF, WEBP) nam trong thu muc theme.
   - So khung cua anh = chieu rong anh / `sprite.frame`. SVG phai co `width`/`height` (khong chi viewBox); PNG ve dung kich thuoc khung.
3. Thay anh. Chi can ghi nhung slot muon doi; slot thieu se lay tu theme mac dinh.

## Slot

| Slot | Noi dung |
|---|---|
| `wall` | o tuong (hang tren cung cua phong) |
| `sign` | bien hieu ghi ten + mo ta phong; ve dang 9 o, vien day `sprite.signSlice` px, phan giua tu keo gian |
| `workspace` | ke sach trong, ghi ten thu muc du an; bam de doi thu muc |
| `floor.<id>` | o san, phong chon bang `room.json` -> `floor` |
| `desk` | ban lam viec, ve de len nua duoi nhan vat |
| `character.<id>.typing` / `.waiting` / `.idle` | nhan vat: dang lam / cho sep / ranh. Anh nhieu khung xep ngang, moi khung `frame` px, tu chay animation |
| `decor.<id>` | do trang tri, dat bang `room.json` -> `decor` |

## Bo tri phong: `departments/<phong>/room.json`

```json
{
  "cols": 8, "rows": 4, "floor": "floor.wood",
  "sign": { "x": 0, "y": 0 }, "workspace": { "x": 7, "y": 0 },
  "decor": [{ "slot": "decor.plant", "x": 0, "y": 1 }],
  "seats": { "dev": { "x": 2, "y": 1, "character": "character.c2" } }
}
```

- `x`, `y`: o tinh tu goc trai tren (hang 0 la tuong).
- Nen de hang 1 trong: bong thoai cua ban hang 2 hien o do, khong de len bien hieu.
- Agent khong co trong `seats` duoc xep tu dong (zic zac tu hang 2) vao o trong. Bo `character` thi chon theo ten Agent.
- Sua xong bam tab Office lai (hoac doi theme) de tai lai.

Theme `default` do Claude ve bang `scripts/gen-theme.mjs` (CC0, dung tu do).
