#!/usr/bin/env node
/**
 * Generate signals.json translations for all supported locales.
 * Run once. Translations are hand-crafted for accuracy.
 */
const fs = require('fs');
const path = require('path');

const translations = {
  ja: {
    "type.securityAlert": "セキュリティアラート",
    "type.breakingChange": "破壊的変更",
    "type.toolDiscovery": "ツール発見",
    "type.techTrend": "技術トレンド",
    "type.learning": "学習",
    "type.competitiveIntel": "競合情報",
    "priority.critical": "重大",
    "priority.alert": "警告",
    "priority.advisory": "注意",
    "priority.watch": "監視",
    "horizon.tactical": "戦術的",
    "horizon.strategic": "戦略的",
    "action.securityReviewStack": "{{title}} を確認 — {{tech}} スタックに影響",
    "action.securityReview": "セキュリティへの影響を確認: {{title}}",
    "action.breakingMigration": "移行パスを確認 — {{tech}} の破壊的変更",
    "action.breakingReview": "破壊的変更を確認: {{title}}",
    "action.toolEvaluateStack": "{{tech}} ワークフローでの評価: {{title}}",
    "action.toolEvaluate": "新しいツールを評価: {{title}}",
    "action.trendStack": "新興トレンド: {{tech}} が注目を集めています — {{title}}",
    "action.trendGeneral": "新興トレンド: {{title}}",
    "action.learnStack": "学習 — {{tech}} リソース: {{title}}",
    "action.learnGeneral": "学習リソース: {{title}}",
    "action.intelStack": "{{tech}} 分野での競合動向: {{title}}",
    "action.intelGeneral": "{{type}}: {{title}}"
  },
  de: {
    "type.securityAlert": "Sicherheitswarnung",
    "type.breakingChange": "Breaking Change",
    "type.toolDiscovery": "Tool-Entdeckung",
    "type.techTrend": "Tech-Trend",
    "type.learning": "Lernressource",
    "type.competitiveIntel": "Wettbewerbsinfo",
    "priority.critical": "kritisch",
    "priority.alert": "Alarm",
    "priority.advisory": "Hinweis",
    "priority.watch": "beobachten",
    "horizon.tactical": "taktisch",
    "horizon.strategic": "strategisch",
    "action.securityReviewStack": "{{title}} prüfen — betrifft Ihren {{tech}}-Stack",
    "action.securityReview": "Sicherheitsauswirkungen prüfen: {{title}}",
    "action.breakingMigration": "Migrationspfad prüfen — {{tech}} Breaking Change",
    "action.breakingReview": "Breaking Change prüfen: {{title}}",
    "action.toolEvaluateStack": "Für Ihren {{tech}}-Workflow bewerten: {{title}}",
    "action.toolEvaluate": "Neues Tool bewerten: {{title}}",
    "action.trendStack": "Aufkommender Trend: {{tech}} gewinnt an Bedeutung — {{title}}",
    "action.trendGeneral": "Aufkommender Trend: {{title}}",
    "action.learnStack": "Lernen — {{tech}}-Ressource: {{title}}",
    "action.learnGeneral": "Lernressource: {{title}}",
    "action.intelStack": "Wettbewerbsbewegung im {{tech}}-Bereich: {{title}}",
    "action.intelGeneral": "{{type}}: {{title}}"
  },
  es: {
    "type.securityAlert": "Alerta de seguridad", "type.breakingChange": "Cambio importante", "type.toolDiscovery": "Herramienta descubierta", "type.techTrend": "Tendencia tecnológica", "type.learning": "Aprendizaje", "type.competitiveIntel": "Intel competitiva",
    "priority.critical": "crítico", "priority.alert": "alerta", "priority.advisory": "aviso", "priority.watch": "vigilar", "horizon.tactical": "táctico", "horizon.strategic": "estratégico",
    "action.securityReviewStack": "Revisar {{title}} — afecta tu stack de {{tech}}", "action.securityReview": "Revisar implicaciones de seguridad: {{title}}", "action.breakingMigration": "Verificar ruta de migración — cambio importante en {{tech}}", "action.breakingReview": "Revisar cambio importante: {{title}}", "action.toolEvaluateStack": "Evaluar para tu flujo de trabajo de {{tech}}: {{title}}", "action.toolEvaluate": "Evaluar nueva herramienta: {{title}}", "action.trendStack": "Tendencia emergente: {{tech}} gana tracción — {{title}}", "action.trendGeneral": "Tendencia emergente: {{title}}", "action.learnStack": "Aprender — recurso de {{tech}}: {{title}}", "action.learnGeneral": "Recurso de aprendizaje: {{title}}", "action.intelStack": "Movimiento competitivo en el espacio {{tech}}: {{title}}", "action.intelGeneral": "{{type}}: {{title}}"
  },
  fr: {
    "type.securityAlert": "Alerte de sécurité", "type.breakingChange": "Changement majeur", "type.toolDiscovery": "Découverte d'outil", "type.techTrend": "Tendance tech", "type.learning": "Apprentissage", "type.competitiveIntel": "Veille concurrentielle",
    "priority.critical": "critique", "priority.alert": "alerte", "priority.advisory": "avis", "priority.watch": "surveiller", "horizon.tactical": "tactique", "horizon.strategic": "stratégique",
    "action.securityReviewStack": "Examiner {{title}} — affecte votre stack {{tech}}", "action.securityReview": "Examiner les implications de sécurité : {{title}}", "action.breakingMigration": "Vérifier le chemin de migration — changement majeur {{tech}}", "action.breakingReview": "Examiner le changement majeur : {{title}}", "action.toolEvaluateStack": "Évaluer pour votre workflow {{tech}} : {{title}}", "action.toolEvaluate": "Évaluer le nouvel outil : {{title}}", "action.trendStack": "Tendance émergente : {{tech}} gagne en popularité — {{title}}", "action.trendGeneral": "Tendance émergente : {{title}}", "action.learnStack": "Apprendre — ressource {{tech}} : {{title}}", "action.learnGeneral": "Ressource d'apprentissage : {{title}}", "action.intelStack": "Mouvement concurrentiel dans l'espace {{tech}} : {{title}}", "action.intelGeneral": "{{type}} : {{title}}"
  },
  zh: {
    "type.securityAlert": "安全警报", "type.breakingChange": "破坏性变更", "type.toolDiscovery": "工具发现", "type.techTrend": "技术趋势", "type.learning": "学习", "type.competitiveIntel": "竞争情报",
    "priority.critical": "严重", "priority.alert": "警报", "priority.advisory": "建议", "priority.watch": "关注", "horizon.tactical": "战术性", "horizon.strategic": "战略性",
    "action.securityReviewStack": "审查 {{title}} — 影响您的 {{tech}} 技术栈", "action.securityReview": "审查安全影响：{{title}}", "action.breakingMigration": "检查迁移路径 — {{tech}} 破坏性变更", "action.breakingReview": "审查破坏性变更：{{title}}", "action.toolEvaluateStack": "评估对您 {{tech}} 工作流的价值：{{title}}", "action.toolEvaluate": "评估新工具：{{title}}", "action.trendStack": "新兴趋势：{{tech}} 正在获得关注 — {{title}}", "action.trendGeneral": "新兴趋势：{{title}}", "action.learnStack": "学习 — {{tech}} 资源：{{title}}", "action.learnGeneral": "学习资源：{{title}}", "action.intelStack": "{{tech}} 领域的竞争动态：{{title}}", "action.intelGeneral": "{{type}}：{{title}}"
  },
  ko: {
    "type.securityAlert": "보안 경고", "type.breakingChange": "주요 변경 사항", "type.toolDiscovery": "도구 발견", "type.techTrend": "기술 트렌드", "type.learning": "학습", "type.competitiveIntel": "경쟁 정보",
    "priority.critical": "심각", "priority.alert": "경고", "priority.advisory": "주의", "priority.watch": "관찰", "horizon.tactical": "전술적", "horizon.strategic": "전략적",
    "action.securityReviewStack": "{{title}} 검토 — {{tech}} 스택에 영향", "action.securityReview": "보안 영향 검토: {{title}}", "action.breakingMigration": "마이그레이션 경로 확인 — {{tech}} 주요 변경", "action.breakingReview": "주요 변경 사항 검토: {{title}}", "action.toolEvaluateStack": "{{tech}} 워크플로에 맞게 평가: {{title}}", "action.toolEvaluate": "새 도구 평가: {{title}}", "action.trendStack": "새로운 트렌드: {{tech}} 주목받는 중 — {{title}}", "action.trendGeneral": "새로운 트렌드: {{title}}", "action.learnStack": "학습 — {{tech}} 리소스: {{title}}", "action.learnGeneral": "학습 리소스: {{title}}", "action.intelStack": "{{tech}} 분야 경쟁 동향: {{title}}", "action.intelGeneral": "{{type}}: {{title}}"
  },
  ar: {
    "type.securityAlert": "تنبيه أمني", "type.breakingChange": "تغيير جذري", "type.toolDiscovery": "اكتشاف أداة", "type.techTrend": "اتجاه تقني", "type.learning": "تعلّم", "type.competitiveIntel": "معلومات تنافسية",
    "priority.critical": "حرج", "priority.alert": "تنبيه", "priority.advisory": "استشاري", "priority.watch": "مراقبة", "horizon.tactical": "تكتيكي", "horizon.strategic": "استراتيجي",
    "action.securityReviewStack": "مراجعة {{title}} — يؤثر على مكدس {{tech}} الخاص بك", "action.securityReview": "مراجعة التأثيرات الأمنية: {{title}}", "action.breakingMigration": "تحقق من مسار الترحيل — تغيير جذري في {{tech}}", "action.breakingReview": "مراجعة التغيير الجذري: {{title}}", "action.toolEvaluateStack": "تقييم لسير عمل {{tech}} الخاص بك: {{title}}", "action.toolEvaluate": "تقييم أداة جديدة: {{title}}", "action.trendStack": "اتجاه ناشئ: {{tech}} يكتسب زخماً — {{title}}", "action.trendGeneral": "اتجاه ناشئ: {{title}}", "action.learnStack": "تعلّم — مورد {{tech}}: {{title}}", "action.learnGeneral": "مورد تعليمي: {{title}}", "action.intelStack": "حركة تنافسية في مجال {{tech}}: {{title}}", "action.intelGeneral": "{{type}}: {{title}}"
  },
  ru: {
    "type.securityAlert": "Предупреждение безопасности", "type.breakingChange": "Критическое изменение", "type.toolDiscovery": "Обнаружение инструмента", "type.techTrend": "Технологический тренд", "type.learning": "Обучение", "type.competitiveIntel": "Конкурентная разведка",
    "priority.critical": "критический", "priority.alert": "тревога", "priority.advisory": "рекомендация", "priority.watch": "наблюдение", "horizon.tactical": "тактический", "horizon.strategic": "стратегический",
    "action.securityReviewStack": "Проверьте {{title}} — затрагивает ваш стек {{tech}}", "action.securityReview": "Проверьте последствия для безопасности: {{title}}", "action.breakingMigration": "Проверьте путь миграции — критическое изменение {{tech}}", "action.breakingReview": "Проверьте критическое изменение: {{title}}", "action.toolEvaluateStack": "Оцените для вашего рабочего процесса {{tech}}: {{title}}", "action.toolEvaluate": "Оцените новый инструмент: {{title}}", "action.trendStack": "Новый тренд: {{tech}} набирает обороты — {{title}}", "action.trendGeneral": "Новый тренд: {{title}}", "action.learnStack": "Обучение — ресурс по {{tech}}: {{title}}", "action.learnGeneral": "Учебный ресурс: {{title}}", "action.intelStack": "Конкурентное движение в сфере {{tech}}: {{title}}", "action.intelGeneral": "{{type}}: {{title}}"
  },
  hi: {
    "type.securityAlert": "सुरक्षा चेतावनी", "type.breakingChange": "महत्वपूर्ण बदलाव", "type.toolDiscovery": "टूल खोज", "type.techTrend": "तकनीकी रुझान", "type.learning": "सीखना", "type.competitiveIntel": "प्रतिस्पर्धी जानकारी",
    "priority.critical": "गंभीर", "priority.alert": "चेतावनी", "priority.advisory": "सलाह", "priority.watch": "निगरानी", "horizon.tactical": "सामरिक", "horizon.strategic": "रणनीतिक",
    "action.securityReviewStack": "{{title}} की समीक्षा करें — आपके {{tech}} स्टैक को प्रभावित करता है", "action.securityReview": "सुरक्षा प्रभावों की समीक्षा करें: {{title}}", "action.breakingMigration": "माइग्रेशन पथ जांचें — {{tech}} में महत्वपूर्ण बदलाव", "action.breakingReview": "महत्वपूर्ण बदलाव की समीक्षा करें: {{title}}", "action.toolEvaluateStack": "अपने {{tech}} वर्कफ़्लो के लिए मूल्यांकन करें: {{title}}", "action.toolEvaluate": "नए टूल का मूल्यांकन करें: {{title}}", "action.trendStack": "उभरता रुझान: {{tech}} लोकप्रिय हो रहा है — {{title}}", "action.trendGeneral": "उभरता रुझान: {{title}}", "action.learnStack": "सीखें — {{tech}} संसाधन: {{title}}", "action.learnGeneral": "शैक्षिक संसाधन: {{title}}", "action.intelStack": "{{tech}} क्षेत्र में प्रतिस्पर्धी गतिविधि: {{title}}", "action.intelGeneral": "{{type}}: {{title}}"
  },
  it: {
    "type.securityAlert": "Avviso di sicurezza", "type.breakingChange": "Modifica importante", "type.toolDiscovery": "Scoperta strumento", "type.techTrend": "Tendenza tecnologica", "type.learning": "Apprendimento", "type.competitiveIntel": "Intel competitiva",
    "priority.critical": "critico", "priority.alert": "allerta", "priority.advisory": "avviso", "priority.watch": "osservare", "horizon.tactical": "tattico", "horizon.strategic": "strategico",
    "action.securityReviewStack": "Verifica {{title}} — interessa il tuo stack {{tech}}", "action.securityReview": "Verifica le implicazioni di sicurezza: {{title}}", "action.breakingMigration": "Controlla il percorso di migrazione — modifica importante {{tech}}", "action.breakingReview": "Verifica la modifica importante: {{title}}", "action.toolEvaluateStack": "Valuta per il tuo flusso {{tech}}: {{title}}", "action.toolEvaluate": "Valuta il nuovo strumento: {{title}}", "action.trendStack": "Tendenza emergente: {{tech}} in crescita — {{title}}", "action.trendGeneral": "Tendenza emergente: {{title}}", "action.learnStack": "Impara — risorsa {{tech}}: {{title}}", "action.learnGeneral": "Risorsa didattica: {{title}}", "action.intelStack": "Movimento competitivo nello spazio {{tech}}: {{title}}", "action.intelGeneral": "{{type}}: {{title}}"
  },
  tr: {
    "type.securityAlert": "Güvenlik uyarısı", "type.breakingChange": "Önemli değişiklik", "type.toolDiscovery": "Araç keşfi", "type.techTrend": "Teknoloji trendi", "type.learning": "Öğrenme", "type.competitiveIntel": "Rekabet istihbaratı",
    "priority.critical": "kritik", "priority.alert": "uyarı", "priority.advisory": "tavsiye", "priority.watch": "izle", "horizon.tactical": "taktik", "horizon.strategic": "stratejik",
    "action.securityReviewStack": "{{title}} inceleyin — {{tech}} yığınınızı etkiliyor", "action.securityReview": "Güvenlik etkilerini inceleyin: {{title}}", "action.breakingMigration": "Geçiş yolunu kontrol edin — {{tech}} önemli değişiklik", "action.breakingReview": "Önemli değişikliği inceleyin: {{title}}", "action.toolEvaluateStack": "{{tech}} iş akışınız için değerlendirin: {{title}}", "action.toolEvaluate": "Yeni aracı değerlendirin: {{title}}", "action.trendStack": "Gelişen trend: {{tech}} ivme kazanıyor — {{title}}", "action.trendGeneral": "Gelişen trend: {{title}}", "action.learnStack": "Öğren — {{tech}} kaynağı: {{title}}", "action.learnGeneral": "Öğrenme kaynağı: {{title}}", "action.intelStack": "{{tech}} alanında rekabet hamlesi: {{title}}", "action.intelGeneral": "{{type}}: {{title}}"
  },
  "pt-BR": {
    "type.securityAlert": "Alerta de segurança", "type.breakingChange": "Mudança importante", "type.toolDiscovery": "Descoberta de ferramenta", "type.techTrend": "Tendência tecnológica", "type.learning": "Aprendizado", "type.competitiveIntel": "Inteligência competitiva",
    "priority.critical": "crítico", "priority.alert": "alerta", "priority.advisory": "aviso", "priority.watch": "observar", "horizon.tactical": "tático", "horizon.strategic": "estratégico",
    "action.securityReviewStack": "Revise {{title}} — afeta sua stack {{tech}}", "action.securityReview": "Revise as implicações de segurança: {{title}}", "action.breakingMigration": "Verifique o caminho de migração — mudança importante no {{tech}}", "action.breakingReview": "Revise a mudança importante: {{title}}", "action.toolEvaluateStack": "Avalie para seu fluxo de trabalho {{tech}}: {{title}}", "action.toolEvaluate": "Avalie a nova ferramenta: {{title}}", "action.trendStack": "Tendência emergente: {{tech}} ganhando tração — {{title}}", "action.trendGeneral": "Tendência emergente: {{title}}", "action.learnStack": "Aprender — recurso {{tech}}: {{title}}", "action.learnGeneral": "Recurso de aprendizado: {{title}}", "action.intelStack": "Movimento competitivo no espaço {{tech}}: {{title}}", "action.intelGeneral": "{{type}}: {{title}}"
  }
};

for (const [lang, data] of Object.entries(translations)) {
  const dir = path.join(__dirname, '..', 'src', 'locales', lang);
  const file = path.join(dir, 'signals.json');
  fs.writeFileSync(file, JSON.stringify(data, null, 2) + '\n');
  console.log(`Wrote ${lang}/signals.json (${Object.keys(data).length} keys)`);
}
console.log(`Done: ${Object.keys(translations).length} languages`);
