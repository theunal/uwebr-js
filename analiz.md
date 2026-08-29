FAZ 9 — Component Props Entegrasyonu (Öncelik: YÜKSEK)
#[component] ve #[derive(Props)] transpiler'a bağlanmıyor. <Card title="x" /> prop'u Element.props'a yazılıyor ama card_component() argüman almıyor.
Yapılacaklar:
1. Transpiler HTML attribute'larından Props struct'ı üretsin
2. #[component] macro'su transpile edilen kodda kullanılsın
3. Component fonksiyonu Props argümanı alsın
4. ComponentFn trait'i güncellensin (veya macro genişletsin)
Karmaşıklık: Yüksek (macro + transpiler + component trait)

FAZ 10 — CSS Düzeltmeleri (Öncelik: ORTA)
Basit düzeltmeler ve orta seviye özellik eksikleri.
Yapılacaklar:
1. overflow: hidden — pipeline.rs:209'daki hardcoded false→ CSS'ten okunan değere çevir (2 satır)
2. Gradient desteği — uwebr-css parser'ına linear-gradient()/radial-gradient() desteği ekle, PaintProps.background'a gradient olarak aktar
3. vw/vh — viewport boyutunu layout engine'e taşı, gerçek viewport'a göre çöz
Karmaşıklık: Basit–Orta

FAZ 11 — Görsel ve Metin İyileştirmeleri (Öncelik: ORTA)
Kullanıcı deneyimini doğrudan etkileyen eksikler.
Yapılacaklar:
1. Image desteği — image crate'i ekle, RenderNodeKind::Image için dekodlama + scene.draw_image()
2. Metin kırpma — text-overflow: ellipsis desteği, clip layer ile taşan metni kırp
3. {@html expr} — runtime HTML alt-parser'ı veya transpile-time çözüm
Karmaşıklık: Yüksek

FAZ 12 — Performans ve Kalite (Öncelik: ORTA)
Üretim hazırlığı için gerekli.
Yapılacaklar:
1. 5 metrik ölçümü — FPS, bellek, binary boyutu, cold start, 1000 node layout
2. Clippy temizliği — 21 uyarının düzeltilmesi (cargo clippy --fix)
3. Benchmark harness'ı — criterion ile benchmark testleri
4. End-to-end test — .uwebr → transpile → render tam yolu testi

Karmaşıklık: Basit
Önerilen Sıralama
Sıra	Faz	Öncelik
1	FAZ 9 — Component Props	YÜKSEK
2	FAZ 10 — CSS Düzeltmeleri	ORTA
3	FAZ 12 — Performans/Kalite	ORTA
4	FAZ 11 — Görsel/Metin	ORTA
Toplam beklenen: ~500+ test, tüm sınırlar giderilmiş, tüm metrikler ölçülmüş.