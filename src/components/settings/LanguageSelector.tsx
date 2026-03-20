import { memo, useCallback, useState } from 'react';
import { useTranslation } from 'react-i18next';

const LANGUAGES = [
  { code: 'en', name: 'English', native: 'English', quality: 'source' as const },
  { code: 'ar', name: 'Arabic', native: 'العربية', quality: 'community' as const },
  { code: 'de', name: 'German', native: 'Deutsch', quality: 'high' as const },
  { code: 'es', name: 'Spanish', native: 'Español', quality: 'high' as const },
  { code: 'fr', name: 'French', native: 'Français', quality: 'high' as const },
  { code: 'hi', name: 'Hindi', native: 'हिन्दी', quality: 'community' as const },
  { code: 'it', name: 'Italian', native: 'Italiano', quality: 'high' as const },
  { code: 'ja', name: 'Japanese', native: '日本語', quality: 'high' as const },
  { code: 'ko', name: 'Korean', native: '한국어', quality: 'high' as const },
  { code: 'pt-BR', name: 'Portuguese (BR)', native: 'Português', quality: 'high' as const },
  { code: 'ru', name: 'Russian', native: 'Русский', quality: 'community' as const },
  { code: 'tr', name: 'Turkish', native: 'Türkçe', quality: 'community' as const },
  { code: 'zh', name: 'Chinese', native: '中文', quality: 'high' as const },
] as const;

// Translation notice — shown in the TARGET language so the reader can understand it
const TRANSLATION_NOTICE: Record<string, string> = {
  ar: 'تمت الترجمة بواسطة الذكاء الاصطناعي. المطور يتحدث الإنجليزية فقط حالياً. إذا وجدت أي خطأ، يرجى إبلاغنا — نسعى لتحقيق دقة 100%.',
  de: 'KI-gestützte Übersetzung. Der Entwickler spricht derzeit nur Englisch. Wenn Sie Fehler finden, lassen Sie es uns bitte wissen — wir streben 100% Genauigkeit an.',
  es: 'Traducción asistida por IA. El desarrollador solo habla inglés actualmente. Si encuentras algún error, por favor avísanos — buscamos una precisión del 100%.',
  fr: 'Traduction assistée par IA. Le développeur ne parle actuellement qu\'anglais. Si vous trouvez des erreurs, merci de nous le signaler — nous visons 100% de précision.',
  hi: 'AI-सहायता प्राप्त अनुवाद। डेवलपर वर्तमान में केवल अंग्रेजी बोलता है। यदि आपको कोई त्रुटि मिले, तो कृपया हमें बताएं — हम 100% सटीकता का लक्ष्य रखते हैं।',
  it: 'Traduzione assistita dall\'IA. Lo sviluppatore parla attualmente solo inglese. Se trovi errori, segnalaceli — puntiamo alla precisione del 100%.',
  ja: 'AI支援による翻訳です。開発者は現在英語のみ対応しています。誤りを見つけた場合はお知らせください — 100%の正確さを目指しています。',
  ko: 'AI 지원 번역입니다. 개발자는 현재 영어만 구사합니다. 오류를 발견하시면 알려주세요 — 100% 정확도를 목표로 합니다.',
  'pt-BR': 'Tradução assistida por IA. O desenvolvedor fala apenas inglês atualmente. Se encontrar erros, avise-nos — buscamos 100% de precisão.',
  ru: 'Перевод выполнен с помощью ИИ. Разработчик в настоящее время говорит только по-английски. Если вы найдёте ошибки, пожалуйста, сообщите нам — мы стремимся к 100% точности.',
  tr: 'Yapay zeka destekli çeviri. Geliştirici şu anda yalnızca İngilizce konuşmaktadır. Hata bulursanız lütfen bize bildirin — %100 doğruluk hedefliyoruz.',
  zh: 'AI辅助翻译。开发者目前仅使用英语。如果您发现任何错误，请告知我们 — 我们致力于实现100%的准确性。',
};

export const LanguageSelector = memo(function LanguageSelector() {
  const { i18n } = useTranslation();
  const currentLang = i18n.language;
  const [showNotice, setShowNotice] = useState(false);

  const handleChange = useCallback(
    (code: string) => {
      i18n.changeLanguage(code);
      localStorage.setItem('4da_language', code);
      // Show translation notice when switching to non-English
      setShowNotice(code !== 'en');
    },
    [i18n],
  );

  const notice = TRANSLATION_NOTICE[currentLang];
  const isNonEnglish = currentLang !== 'en';

  return (
    <div className="bg-bg-tertiary rounded-lg p-4 border border-border">
      <h3 className="text-sm font-medium text-white mb-1">Language</h3>
      <p className="text-xs text-text-muted mb-3">
        {LANGUAGES.length} languages supported. Changes apply immediately.
      </p>
      <div className="grid grid-cols-2 gap-1.5">
        {LANGUAGES.map((lang) => (
          <button
            key={lang.code}
            onClick={() => handleChange(lang.code)}
            className={`flex items-center gap-2 px-3 py-2 rounded-lg text-left transition-all ${
              currentLang === lang.code
                ? 'bg-white/10 border border-white/20 text-white'
                : 'border border-transparent text-text-secondary hover:bg-white/5 hover:text-white'
            }`}
          >
            <span className="text-xs font-medium w-16 shrink-0">{lang.native}</span>
            <span className="text-[10px] text-text-muted">{lang.name}</span>
            {currentLang === lang.code && (
              <span className="ml-auto w-1.5 h-1.5 rounded-full bg-[#22C55E]" />
            )}
          </button>
        ))}
      </div>

      {/* Translation quality notice — shown in the user's chosen language */}
      {isNonEnglish && (showNotice || true) && notice && (
        <div className="mt-3 px-3 py-2.5 rounded-lg bg-[#D4AF37]/5 border border-[#D4AF37]/15">
          <p className="text-[11px] text-[#D4AF37]/80 leading-relaxed">
            {notice}
          </p>
          <a
            href="https://github.com/4da-systems/4da/issues"
            target="_blank"
            rel="noopener noreferrer"
            className="inline-block mt-1.5 text-[10px] text-[#D4AF37] hover:text-[#D4AF37]/80 transition-colors"
          >
            Report translation issue →
          </a>
        </div>
      )}
    </div>
  );
});
