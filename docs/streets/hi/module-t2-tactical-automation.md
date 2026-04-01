# मॉड्यूल T: टैक्टिकल ऑटोमेशन

**STREETS डेवलपर इनकम कोर्स — पेड मॉड्यूल**
*सप्ताह 12-13 | 6 पाठ | डिलिवरेबल: एक ऑटोमेटेड पाइपलाइन जो मूल्य उत्पन्न करती है*

> "LLMs, एजेंट्स, MCP, और cron जॉब्स फोर्स मल्टीप्लायर के रूप में।"

---

आपके रेवेन्यू इंजन चल रहे हैं। आपके कस्टमर हैं। आपके पास ऐसी प्रक्रियाएं हैं जो काम करती हैं। और आप अपना 60-70% समय वही चीजें बार-बार करने में बिता रहे हैं: इनपुट प्रोसेस करना, आउटपुट फॉर्मेट करना, मॉनिटर चेक करना, अपडेट भेजना, क्यू रिव्यू करना।

वह समय आपका सबसे महंगा संसाधन है, और आप इसे उन कामों पर जला रहे हैं जो {= regional.currency_symbol | fallback("$") =}5/माह का VPS संभाल सकता है।

{@ insight hardware_benchmark @}

यह मॉड्यूल व्यवस्थित रूप से आपको लूप से हटाने के बारे में है — पूरी तरह नहीं (यह एक जाल है जिसे हम पाठ 5 में कवर करेंगे), बल्कि उन 80% काम से जिन्हें आपके निर्णय की आवश्यकता नहीं है। परिणाम: आपकी इनकम स्ट्रीम आपके सोते समय, आपकी डे जॉब के दौरान, अगली चीज बनाते समय रेवेन्यू उत्पन्न करती हैं।

इन दो सप्ताहों के अंत तक, आपके पास होगा:

- ऑटोमेशन के चार स्तरों की स्पष्ट समझ और आज आप कहां खड़े हैं
- आपकी इंफ्रास्ट्रक्चर पर चलने वाले cron जॉब्स और शेड्यूल्ड ऑटोमेशन
- कम से कम एक LLM-पावर्ड पाइपलाइन जो बिना आपकी भागीदारी के इनपुट प्रोसेस करती है
- एजेंट-बेस्ड सिस्टम की समझ और वे कब आर्थिक रूप से समझदार होते हैं
- एक ह्यूमन-इन-द-लूप फ्रेमवर्क ताकि ऑटोमेशन आपकी प्रतिष्ठा को नष्ट न करे
- एक पूर्ण, डिप्लॉय किया हुआ पाइपलाइन जो बिना आपकी सक्रिय भागीदारी के मूल्य उत्पन्न करता है

{? if stack.primary ?}
आपकी प्राथमिक स्टैक {= stack.primary | fallback("आपकी प्राथमिक स्टैक") =} है, इसलिए आगे के ऑटोमेशन उदाहरण उस इकोसिस्टम में अनुकूलित करने पर सबसे अधिक सीधे लागू होंगे। अधिकांश उदाहरण पोर्टेबिलिटी के लिए Python उपयोग करते हैं, लेकिन पैटर्न किसी भी भाषा में काम करते हैं।
{? endif ?}

यह कोर्स का सबसे कोड-भारी मॉड्यूल है। जो कुछ आगे है उसका कम से कम आधा रनेबल कोड है। इसे कॉपी करें, अनुकूलित करें, डिप्लॉय करें।

चलिए ऑटोमेट करते हैं।

---

## पाठ 1: ऑटोमेशन पिरामिड

*"अधिकांश डेवलपर लेवल 1 पर ऑटोमेट करते हैं। पैसा लेवल 3 पर है।"*

### चार स्तर

आपकी इनकम स्टैक में हर ऑटोमेशन इस पिरामिड पर कहीं आता है:

```
┌───────────────────────────────┐
│  लेवल 4: ऑटोनॉमस एजेंट्स    │  ← आपके लिए निर्णय लेता है
│  (AI निर्णय लेता है और कार्य करता है)│
├───────────────────────────────┤
│  लेवल 3: इंटेलिजेंट          │  ← पैसा यहां है
│  पाइपलाइन (LLM-पावर्ड)      │
├───────────────────────────────┤
│  लेवल 2: शेड्यूल्ड           │  ← अधिकांश डेवलपर यहां रुकते हैं
│  ऑटोमेशन (cron + स्क्रिप्ट)  │
├───────────────────────────────┤
│  लेवल 1: टेम्पलेट के साथ     │  ← जहां अधिकांश डेवलपर हैं
│  मैनुअल (कॉपी-पेस्ट)        │
└───────────────────────────────┘
```

आइए स्पष्ट करें कि प्रैक्टिस में प्रत्येक स्तर कैसा दिखता है।

### लेवल 1: टेम्पलेट के साथ मैनुअल

आप काम करते हैं, लेकिन आपके पास चेकलिस्ट, टेम्पलेट और स्निपेट हैं चीजों को तेज करने के लिए।

**उदाहरण:**
- आप पहले से भरे frontmatter के साथ markdown टेम्पलेट का उपयोग करके ब्लॉग पोस्ट लिखते हैं
- आप पिछले महीने की इनवॉइस डुप्लिकेट करके और नंबर बदलकर क्लाइंट्स को बिल करते हैं
- आप सेव किए गए रिप्लाइज का उपयोग करके सपोर्ट ईमेल का जवाब देते हैं
- आप मैन्युअली डिप्लॉय कमांड चलाकर कंटेंट पब्लिश करते हैं

**समय की लागत:** आउटपुट की प्रति यूनिट आपका 100% समय।
**एरर रेट:** मध्यम — आप इंसान हैं, थके होने पर गलतियां करते हैं।
**स्केल सीलिंग:** आप। आपके घंटे। बस इतना ही।

अधिकांश डेवलपर यहां रहते हैं और उन्हें यह भी एहसास नहीं होता कि उनके ऊपर एक पिरामिड है।

### लेवल 2: शेड्यूल्ड ऑटोमेशन

स्क्रिप्ट शेड्यूल पर चलती हैं। आपने लॉजिक एक बार लिखा। यह आपके बिना एक्सीक्यूट होता है।

**उदाहरण:**
- एक cron जॉब जो आपके RSS फीड को चेक करती है और नए आर्टिकल सोशल मीडिया पर पोस्ट करती है
- एक GitHub Action जो हर सुबह 6 बजे आपकी साइट बिल्ड और डिप्लॉय करती है
- एक स्क्रिप्ट जो हर घंटे कॉम्पिटिटर प्राइसिंग चेक करती है और बदलाव लॉग करती है
- एक डेली डेटाबेस बैकअप जो सुबह 3 बजे चलता है

**समय की लागत:** शून्य निरंतर (1-4 घंटे के शुरुआती सेटअप के बाद)।
**एरर रेट:** कम — डिटर्मिनिस्टिक, हर बार वही लॉजिक।
**स्केल सीलिंग:** जितने टास्क आपकी मशीन शेड्यूल कर सकती है। सैकड़ों।

यहां अधिकांश तकनीकी डेवलपर पहुंचते हैं। यह आरामदायक है। लेकिन इसकी एक कठोर सीमा है: यह केवल डिटर्मिनिस्टिक लॉजिक वाले कामों को संभाल सकता है। अगर काम में जज्मेंट चाहिए, तो आप फंसे हुए हैं।

### लेवल 3: इंटेलिजेंट पाइपलाइन

स्क्रिप्ट शेड्यूल पर चलती हैं, लेकिन इनमें एक LLM शामिल है जो जज्मेंट कॉल्स संभालता है।

**उदाहरण:**
- RSS फीड इंजेस्ट होती हैं, LLM हर आर्टिकल को सारांशित करता है, न्यूजलेटर ड्राफ्ट करता है, आप 10 मिनट रिव्यू करके सेंड करते हैं
- कस्टमर फीडबैक ईमेल सेंटिमेंट और अर्जेंसी के अनुसार क्लासिफाई होती हैं, प्री-ड्राफ्टेड रिस्पॉन्स आपकी अप्रूवल के लिए क्यू में लगते हैं
- आपके निश में नई जॉब पोस्टिंग स्क्रैप होती हैं, LLM रेलेवेंस इवैल्यूएट करता है, आपको 200 लिस्टिंग स्कैन करने के बजाय 5 अवसरों का डेली डाइजेस्ट मिलता है
- कॉम्पिटिटर ब्लॉग पोस्ट मॉनिटर होती हैं, LLM प्रमुख प्रोडक्ट बदलाव एक्सट्रैक्ट करता है, आपको वीकली कॉम्पिटिटिव इंटेलिजेंस रिपोर्ट मिलती है

**समय की लागत:** मैनुअल समय का 10-20%। आप बनाने के बजाय रिव्यू और अप्रूव करते हैं।
**एरर रेट:** क्लासिफिकेशन टास्क के लिए कम, जनरेशन के लिए मध्यम (इसीलिए आप रिव्यू करते हैं)।
**स्केल सीलिंग:** प्रति दिन हजारों आइटम। आपकी बॉटलनेक API कॉस्ट है, आपका समय नहीं।

**पैसा यहां है।** लेवल 3 एक व्यक्ति को ऐसी इनकम स्ट्रीम चलाने देता है जिनके लिए सामान्यतः 3-5 लोगों की टीम चाहिए।

### लेवल 4: ऑटोनॉमस एजेंट्स

AI सिस्टम जो आपकी भागीदारी के बिना ऑब्जर्व, डिसाइड और एक्ट करते हैं।

**उदाहरण:**
- एक एजेंट जो आपके SaaS मेट्रिक्स मॉनिटर करता है, साइनअप में गिरावट डिटेक्ट करता है, प्राइसिंग बदलाव A/B टेस्ट करता है, और अगर काम नहीं करता तो रिवर्ट करता है
- एक सपोर्ट एजेंट जो टियर 1 कस्टमर प्रश्नों को पूरी तरह ऑटोनॉमसली हैंडल करता है, केवल जटिल मुद्दों के लिए आपको एस्केलेट करता है
- एक कंटेंट एजेंट जो ट्रेंडिंग टॉपिक्स आइडेंटिफाई करता है, ड्राफ्ट जनरेट करता है, पब्लिकेशन शेड्यूल करता है, और परफॉर्मेंस मॉनिटर करता है

**समय की लागत:** हैंडल किए गए केसों के लिए लगभग शून्य। आप मेट्रिक्स रिव्यू करते हैं, इंडिविजुअल एक्शन नहीं।
**एरर रेट:** पूरी तरह आपके गार्डरेल्स पर निर्भर। उनके बिना: ऊंचा। अच्छे गार्डरेल्स के साथ: संकीर्ण डोमेन के लिए आश्चर्यजनक रूप से कम।
**स्केल सीलिंग:** एजेंट के स्कोप में टास्क के लिए प्रभावी रूप से असीमित।

लेवल 4 वास्तविक और प्राप्त करने योग्य है, लेकिन यह वह जगह नहीं है जहां आप शुरू करते हैं। और जैसा कि हम पाठ 5 में कवर करेंगे, खराब तरीके से लागू किए गए पूरी तरह ऑटोनॉमस कस्टमर-फेसिंग एजेंट आपकी प्रतिष्ठा के लिए खतरनाक हैं।

> **सीधी बात:** अगर आप अभी लेवल 1 पर हैं, तो लेवल 4 पर कूदने की कोशिश न करें। आप हफ्ते बिताएंगे एक "ऑटोनॉमस एजेंट" बनाने में जो प्रोडक्शन में टूट जाता है और कस्टमर ट्रस्ट को नुकसान पहुंचाता है। पिरामिड एक बार में एक लेवल चढ़ें। लेवल 2 एक दोपहर का काम है। लेवल 3 वीकेंड प्रोजेक्ट है। लेवल 4 तब आता है जब आपके पास लेवल 3 एक महीने से विश्वसनीय रूप से चल रहा हो।

### सेल्फ-असेसमेंट: आप कहां हैं?

अपनी प्रत्येक इनकम स्ट्रीम के लिए, ईमानदारी से रेट करें:

| इनकम स्ट्रीम | वर्तमान लेवल | घंटे/सप्ताह खर्च | ऑटोमेट कर सकते हैं |
|-------------|-------------|-----------------|-------------------|
| [जैसे, न्यूजलेटर] | [1-4] | [X] घंटे | [लक्ष्य लेवल] |
| [जैसे, क्लाइंट प्रोसेसिंग] | [1-4] | [X] घंटे | [लक्ष्य लेवल] |
| [जैसे, सोशल मीडिया] | [1-4] | [X] घंटे | [लक्ष्य लेवल] |
| [जैसे, सपोर्ट] | [1-4] | [X] घंटे | [लक्ष्य लेवल] |

सबसे महत्वपूर्ण कॉलम "घंटे/सप्ताह खर्च" है। सबसे अधिक घंटे और सबसे कम लेवल वाली स्ट्रीम आपका पहला ऑटोमेशन टारगेट है। वही सबसे अधिक ROI देगी।

### प्रत्येक स्तर की अर्थव्यवस्था

मान लीजिए आपके पास एक इनकम स्ट्रीम है जो आपके 10 घंटे/सप्ताह लेती है और {= regional.currency_symbol | fallback("$") =}2,000/माह जनरेट करती है:

| लेवल | आपका समय | आपकी प्रभावी रेट | ऑटोमेशन लागत |
|------|---------|----------------|-------------|
| लेवल 1 | 10 घंटे/सप्ताह | $50/घंटा | $0 |
| लेवल 2 | 3 घंटे/सप्ताह | $167/घंटा | $5/माह (VPS) |
| लेवल 3 | 1 घंटा/सप्ताह | $500/घंटा | $30-50/माह (API) |
| लेवल 4 | 0.5 घंटा/सप्ताह | $1,000/घंटा | $50-100/माह (API + कंप्यूट) |

लेवल 1 से लेवल 3 पर जाने से आपका रेवेन्यू नहीं बदलता। यह आपकी प्रभावी प्रति घंटा दर $50 से $500 कर देता है। और वे 9 मुक्त घंटे? वे अगली इनकम स्ट्रीम बनाने या मौजूदा को सुधारने में जाते हैं।

> **सामान्य गलती:** अपनी सबसे कम रेवेन्यू स्ट्रीम पहले ऑटोमेट करना क्योंकि यह "आसान" है। नहीं। उस स्ट्रीम को ऑटोमेट करें जो अपने रेवेन्यू के सापेक्ष सबसे अधिक घंटे खाती है। वहां ROI है।

### आपकी बारी

1. ऊपर का सेल्फ-असेसमेंट टेबल अपनी हर इनकम स्ट्रीम (या प्लान्ड स्ट्रीम) के लिए भरें।
2. अपना सबसे अधिक ROI ऑटोमेशन टारगेट पहचानें: सबसे अधिक घंटे और सबसे कम ऑटोमेशन लेवल वाली स्ट्रीम।
3. उस स्ट्रीम में सबसे अधिक समय खाने वाले 3 काम लिखें। आप पहले वाले को पाठ 2 में ऑटोमेट करेंगे।

---

## पाठ 2: लेवल 1 से 2 — शेड्यूल्ड ऑटोमेशन

*"cron 1975 से है। अभी भी काम करता है। इस्तेमाल करें।"*

### Cron जॉब बेसिक्स

{? if computed.os_family == "windows" ?}
आप Windows पर हैं, इसलिए cron आपके सिस्टम में नेटिव नहीं है। आपके पास दो विकल्प हैं: WSL (Windows Subsystem for Linux) का उपयोग करें असली cron पाने के लिए, या Windows Task Scheduler उपयोग करें (नीचे कवर किया गया)। अगर आप इसमें सहज हैं तो WSL की सिफारिश है — इस पाठ के सभी cron उदाहरण सीधे WSL में काम करते हैं। अगर आप नेटिव Windows पसंद करते हैं, इसके बाद Task Scheduler सेक्शन पर जाएं।
{? endif ?}

हां, 2026 में भी, शेड्यूल्ड टास्क के लिए cron राजा है। यह विश्वसनीय है, हर जगह है, और इसे क्लाउड अकाउंट, SaaS सब्सक्रिप्शन, या YAML स्कीमा की जरूरत नहीं जिसे आपको हर बार Google करना पड़े।

**cron सिंटैक्स 30 सेकंड में:**

```
┌───────── मिनट (0-59)
│ ┌───────── घंटा (0-23)
│ │ ┌───────── महीने का दिन (1-31)
│ │ │ ┌───────── महीना (1-12)
│ │ │ │ ┌───────── सप्ताह का दिन (0-7, 0 और 7 = रविवार)
│ │ │ │ │
* * * * *  कमांड
```

**सामान्य शेड्यूल:**

```bash
# हर घंटे
0 * * * *  /path/to/script.sh

# हर दिन सुबह 6 बजे
0 6 * * *  /path/to/script.sh

# हर सोमवार सुबह 9 बजे
0 9 * * 1  /path/to/script.sh

# हर 15 मिनट
*/15 * * * *  /path/to/script.sh

# हर महीने की पहली तारीख मध्यरात्रि
0 0 1 * *  /path/to/script.sh
```

**cron जॉब सेटअप करना:**

```bash
# अपना crontab एडिट करें
crontab -e

# मौजूदा cron जॉब लिस्ट करें
crontab -l

# महत्वपूर्ण: हमेशा ऊपर एनवायरनमेंट वेरिएबल्स सेट करें
# cron मिनिमल एनवायरनमेंट के साथ चलता है — PATH में आपके टूल्स शामिल नहीं हो सकते
SHELL=/bin/bash
PATH=/usr/local/bin:/usr/bin:/bin
HOME=/home/youruser

# आउटपुट लॉग करें ताकि आप फेलियर डीबग कर सकें
0 6 * * * /home/youruser/scripts/daily-report.sh >> /home/youruser/logs/daily-report.log 2>&1
```

> **सामान्य गलती:** एक स्क्रिप्ट लिखना जो मैन्युअली चलाने पर बिल्कुल सही काम करती है, फिर cron में चुपचाप फेल हो जाती है क्योंकि cron आपकी `.bashrc` या `.zshrc` लोड नहीं करता। cron स्क्रिप्ट में हमेशा एब्सोल्यूट पाथ उपयोग करें। हमेशा अपने crontab के ऊपर `PATH` सेट करें। हमेशा आउटपुट को लॉग फाइल में रीडायरेक्ट करें।

### क्लाउड शेड्यूलर जब cron पर्याप्त नहीं है

अगर आपकी मशीन 24/7 चालू नहीं है, या आपको कुछ अधिक मजबूत चाहिए, क्लाउड शेड्यूलर उपयोग करें:

**GitHub Actions (पब्लिक रेपो के लिए मुफ्त, प्राइवेट पर 2,000 मिनट/माह):**

```yaml
# .github/workflows/scheduled-task.yml
name: Daily Content Publisher

on:
  schedule:
    # हर दिन सुबह 6 बजे UTC
    - cron: '0 6 * * *'
  # टेस्टिंग के लिए मैनुअल ट्रिगर की अनुमति
  workflow_dispatch:

jobs:
  publish:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Set up Node.js
        uses: actions/setup-node@v4
        with:
          node-version: '20'

      - name: Install dependencies
        run: npm ci

      - name: Run publisher
        env:
          CMS_API_KEY: ${{ secrets.CMS_API_KEY }}
          SOCIAL_TOKEN: ${{ secrets.SOCIAL_TOKEN }}
        run: node scripts/publish-scheduled-content.js
```

**Vercel Cron (Hobby प्लान पर मुफ्त, प्रति दिन 1; Pro प्लान: अनलिमिटेड):**

```typescript
// api/cron/daily-report.ts
// Vercel cron एंडपॉइंट — vercel.json में शेड्यूल कॉन्फ़िगर करें

import type { NextRequest } from 'next/server';

export const config = {
  runtime: 'edge',
};

export default async function handler(req: NextRequest) {
  // वेरिफाई करें कि यह वाकई Vercel कॉल कर रहा है, कोई रैंडम HTTP रिक्वेस्ट नहीं
  const authHeader = req.headers.get('authorization');
  if (authHeader !== `Bearer ${process.env.CRON_SECRET}`) {
    return new Response('Unauthorized', { status: 401 });
  }

  // आपका ऑटोमेशन लॉजिक यहां
  const report = await generateDailyReport();
  await sendToSlack(report);

  return new Response('OK', { status: 200 });
}
```

```json
// vercel.json
{
  "crons": [
    {
      "path": "/api/cron/daily-report",
      "schedule": "0 6 * * *"
    }
  ]
}
```

### अभी बनाने लायक असली ऑटोमेशन

यहां पांच ऑटोमेशन हैं जिन्हें आप आज लागू कर सकते हैं। हर एक 30-60 मिनट लेती है और साप्ताहिक मैनुअल काम के घंटे हटाती है।

#### ऑटोमेशन 1: शेड्यूल पर ऑटो-पब्लिश कंटेंट

आप पहले से ब्लॉग पोस्ट लिखते हैं। यह स्क्रिप्ट उन्हें शेड्यूल्ड समय पर पब्लिश करती है।

```python
#!/usr/bin/env python3
"""
scheduled_publisher.py — markdown पोस्ट उनकी शेड्यूल्ड तारीख पर पब्लिश करें।
cron से डेली चलाएं: 0 6 * * * python3 /path/to/scheduled_publisher.py
"""

import os
import json
import glob
import requests
from datetime import datetime, timezone
from pathlib import Path

CONTENT_DIR = os.path.expanduser("~/income/content/posts")
PUBLISHED_LOG = os.path.expanduser("~/income/content/published.json")

# आपका CMS API एंडपॉइंट (Hashnode, Dev.to, Ghost, आदि)
CMS_API_URL = os.environ.get("CMS_API_URL", "https://api.example.com/posts")
CMS_API_KEY = os.environ.get("CMS_API_KEY", "")

def load_published():
    """पहले से पब्लिश हो चुकी पोस्ट फाइलनेम की लिस्ट लोड करें।"""
    try:
        with open(PUBLISHED_LOG, "r") as f:
            return set(json.load(f))
    except (FileNotFoundError, json.JSONDecodeError):
        return set()

def save_published(published: set):
    """पब्लिश हो चुकी पोस्ट फाइलनेम की लिस्ट सेव करें।"""
    with open(PUBLISHED_LOG, "w") as f:
        json.dump(sorted(published), f, indent=2)

def parse_frontmatter(filepath: str) -> dict:
    """markdown फाइल से YAML-स्टाइल frontmatter एक्सट्रैक्ट करें।"""
    with open(filepath, "r", encoding="utf-8") as f:
        content = f.read()

    if not content.startswith("---"):
        return {}

    parts = content.split("---", 2)
    if len(parts) < 3:
        return {}

    metadata = {}
    for line in parts[1].strip().split("\n"):
        if ":" in line:
            key, value = line.split(":", 1)
            metadata[key.strip()] = value.strip().strip('"').strip("'")

    metadata["body"] = parts[2].strip()
    return metadata

def should_publish(metadata: dict) -> bool:
    """चेक करें कि क्या पोस्ट आज पब्लिश होनी चाहिए।"""
    publish_date = metadata.get("publish_date", "")
    if not publish_date:
        return False

    try:
        scheduled = datetime.strptime(publish_date, "%Y-%m-%d").date()
        return scheduled <= datetime.now(timezone.utc).date()
    except ValueError:
        return False

def publish_post(metadata: dict) -> bool:
    """अपने CMS API पर पोस्ट पब्लिश करें।"""
    payload = {
        "title": metadata.get("title", "Untitled"),
        "content": metadata.get("body", ""),
        "tags": metadata.get("tags", "").split(","),
        "status": "published"
    }

    try:
        response = requests.post(
            CMS_API_URL,
            json=payload,
            headers={
                "Authorization": f"Bearer {CMS_API_KEY}",
                "Content-Type": "application/json"
            },
            timeout=30
        )
        response.raise_for_status()
        print(f"  पब्लिश हुआ: {metadata.get('title')}")
        return True
    except requests.RequestException as e:
        print(f"  विफल: {metadata.get('title')} — {e}")
        return False

def main():
    published = load_published()
    posts = glob.glob(os.path.join(CONTENT_DIR, "*.md"))

    print(f"{len(posts)} पोस्ट चेक हो रही हैं...")

    for filepath in sorted(posts):
        filename = os.path.basename(filepath)

        if filename in published:
            continue

        metadata = parse_frontmatter(filepath)
        if not metadata:
            continue

        if should_publish(metadata):
            if publish_post(metadata):
                published.add(filename)

    save_published(published)
    print(f"कुल पब्लिश: {len(published)}")

if __name__ == "__main__":
    main()
```

**आपकी markdown पोस्ट ऐसी दिखती हैं:**

```markdown
---
title: "How to Deploy Ollama Behind Nginx"
publish_date: "2026-03-15"
tags: ollama, deployment, nginx
---

आपकी पोस्ट का कंटेंट यहां...
```

जब इंस्पिरेशन आए तब पोस्ट लिखें। तारीख सेट करें। स्क्रिप्ट बाकी संभाल लेती है।

#### ऑटोमेशन 2: नए कंटेंट पर ऑटो-पोस्ट सोशल मीडिया

जब आपका ब्लॉग कुछ नया पब्लिश करता है, यह ऑटोमैटिकली Twitter/X और Bluesky पर पोस्ट करता है।

```python
#!/usr/bin/env python3
"""
social_poster.py — नया कंटेंट पब्लिश होने पर सोशल प्लेटफॉर्म पर पोस्ट करें।
हर 30 मिनट चलाएं: */30 * * * * python3 /path/to/social_poster.py
"""

import os
import json
import hashlib
import requests
from datetime import datetime

FEED_URL = os.environ.get("RSS_FEED_URL", "https://yourblog.com/rss.xml")
POSTED_LOG = os.path.expanduser("~/income/logs/social_posted.json")
BLUESKY_HANDLE = os.environ.get("BLUESKY_HANDLE", "")
BLUESKY_APP_PASSWORD = os.environ.get("BLUESKY_APP_PASSWORD", "")

def load_posted() -> set:
    try:
        with open(POSTED_LOG, "r") as f:
            return set(json.load(f))
    except (FileNotFoundError, json.JSONDecodeError):
        return set()

def save_posted(posted: set):
    os.makedirs(os.path.dirname(POSTED_LOG), exist_ok=True)
    with open(POSTED_LOG, "w") as f:
        json.dump(sorted(posted), f, indent=2)

def get_rss_items(feed_url: str) -> list:
    """RSS फीड पार्स करें और आइटम की लिस्ट रिटर्न करें।"""
    import xml.etree.ElementTree as ET

    response = requests.get(feed_url, timeout=30)
    response.raise_for_status()
    root = ET.fromstring(response.content)

    items = []
    for item in root.findall(".//item"):
        title = item.findtext("title", "")
        link = item.findtext("link", "")
        description = item.findtext("description", "")
        item_id = hashlib.md5(link.encode()).hexdigest()
        items.append({
            "id": item_id,
            "title": title,
            "link": link,
            "description": description[:200]
        })
    return items

def post_to_bluesky(text: str):
    """AT Protocol से Bluesky पर पोस्ट करें।"""
    # स्टेप 1: सेशन बनाएं
    session_resp = requests.post(
        "https://bsky.social/xrpc/com.atproto.server.createSession",
        json={
            "identifier": BLUESKY_HANDLE,
            "password": BLUESKY_APP_PASSWORD
        },
        timeout=30
    )
    session_resp.raise_for_status()
    session = session_resp.json()

    # स्टेप 2: पोस्ट बनाएं
    post_resp = requests.post(
        "https://bsky.social/xrpc/com.atproto.repo.createRecord",
        headers={"Authorization": f"Bearer {session['accessJwt']}"},
        json={
            "repo": session["did"],
            "collection": "app.bsky.feed.post",
            "record": {
                "$type": "app.bsky.feed.post",
                "text": text,
                "createdAt": datetime.utcnow().isoformat() + "Z"
            }
        },
        timeout=30
    )
    post_resp.raise_for_status()
    print(f"  Bluesky पर पोस्ट हुआ: {text[:60]}...")

def main():
    posted = load_posted()
    items = get_rss_items(FEED_URL)

    for item in items:
        if item["id"] in posted:
            continue

        # सोशल पोस्ट फॉर्मेट करें
        text = f"{item['title']}\n\n{item['link']}"

        # Bluesky की 300 कैरेक्टर लिमिट है
        if len(text) > 300:
            text = f"{item['title'][:240]}...\n\n{item['link']}"

        try:
            post_to_bluesky(text)
            posted.add(item["id"])
        except Exception as e:
            print(f"  पोस्ट विफल: {e}")

    save_posted(posted)

if __name__ == "__main__":
    main()
```

लागत: $0। आपकी मशीन या मुफ्त GitHub Action पर चलता है।

#### ऑटोमेशन 3: कॉम्पिटिटर प्राइस मॉनिटर

जब कोई कॉम्पिटिटर अपनी प्राइसिंग बदले तो तुरंत जानें। अब हर हफ्ते मैन्युअली चेक करने की जरूरत नहीं।

```python
#!/usr/bin/env python3
"""
price_monitor.py — कॉम्पिटिटर प्राइसिंग पेज में बदलाव मॉनिटर करें।
हर 6 घंटे चलाएं: 0 */6 * * * python3 /path/to/price_monitor.py
"""

import os
import json
import hashlib
import requests
from datetime import datetime
from pathlib import Path

MONITOR_DIR = os.path.expanduser("~/income/monitors")
ALERT_WEBHOOK = os.environ.get("SLACK_WEBHOOK_URL", "")  # या Discord, ईमेल, आदि

COMPETITORS = [
    {
        "name": "CompetitorA",
        "url": "https://competitor-a.com/pricing",
        "css_selector": None  # पूरे पेज मॉनिटरिंग के लिए; विशिष्ट एलिमेंट के लिए सेलेक्टर उपयोग करें
    },
    {
        "name": "CompetitorB",
        "url": "https://competitor-b.com/pricing",
        "css_selector": None
    },
]

def get_page_hash(url: str) -> tuple[str, str]:
    """पेज फेच करें और उसका कंटेंट हैश और टेक्स्ट एक्सरप्ट रिटर्न करें।"""
    headers = {
        "User-Agent": "Mozilla/5.0 (compatible; PriceMonitor/1.0)"
    }
    response = requests.get(url, headers=headers, timeout=30)
    response.raise_for_status()
    content = response.text
    content_hash = hashlib.sha256(content.encode()).hexdigest()
    # कॉन्टेक्स्ट के लिए विजिबल टेक्स्ट के पहले 500 कैरेक्टर लें
    excerpt = content[:500]
    return content_hash, excerpt

def load_state(name: str) -> dict:
    state_file = os.path.join(MONITOR_DIR, f"{name}.json")
    try:
        with open(state_file, "r") as f:
            return json.load(f)
    except (FileNotFoundError, json.JSONDecodeError):
        return {}

def save_state(name: str, state: dict):
    os.makedirs(MONITOR_DIR, exist_ok=True)
    state_file = os.path.join(MONITOR_DIR, f"{name}.json")
    with open(state_file, "w") as f:
        json.dump(state, f, indent=2)

def send_alert(message: str):
    """Slack webhook से अलर्ट भेजें (Discord, ईमेल, आदि से बदल सकते हैं)।"""
    if not ALERT_WEBHOOK:
        print(f"अलर्ट (कोई webhook कॉन्फ़िगर नहीं): {message}")
        return

    requests.post(ALERT_WEBHOOK, json={"text": message}, timeout=10)

def main():
    for competitor in COMPETITORS:
        name = competitor["name"]
        url = competitor["url"]

        try:
            current_hash, excerpt = get_page_hash(url)
        except Exception as e:
            print(f"  {name} फेच विफल: {e}")
            continue

        state = load_state(name)
        previous_hash = state.get("hash", "")

        if previous_hash and current_hash != previous_hash:
            alert_msg = (
                f"प्राइसिंग बदलाव डिटेक्ट: {name}\n"
                f"URL: {url}\n"
                f"बदला: {datetime.utcnow().isoformat()}Z\n"
                f"पिछला हैश: {previous_hash[:12]}...\n"
                f"नया हैश: {current_hash[:12]}...\n"
                f"मैन्युअली चेक करें।"
            )
            send_alert(alert_msg)
            print(f"  बदलाव: {name}")
        else:
            print(f"  कोई बदलाव नहीं: {name}")

        save_state(name, {
            "hash": current_hash,
            "last_checked": datetime.utcnow().isoformat() + "Z",
            "url": url,
            "excerpt": excerpt[:200]
        })

if __name__ == "__main__":
    main()
```

#### ऑटोमेशन 4: वीकली रेवेन्यू रिपोर्ट

हर सोमवार सुबह, यह आपके रेवेन्यू डेटा से रिपोर्ट जनरेट करता है और आपको ईमेल करता है।

```python
#!/usr/bin/env python3
"""
weekly_report.py — आपकी ट्रैकिंग स्प्रेडशीट/डेटाबेस से वीकली रेवेन्यू रिपोर्ट जनरेट करें।
सोमवार सुबह 7 बजे चलाएं: 0 7 * * 1 python3 /path/to/weekly_report.py
"""

import os
import json
import sqlite3
import smtplib
from email.mime.text import MIMEText
from datetime import datetime, timedelta

DB_PATH = os.path.expanduser("~/income/data/revenue.db")
EMAIL_TO = os.environ.get("REPORT_EMAIL", "you@example.com")
SMTP_HOST = os.environ.get("SMTP_HOST", "smtp.gmail.com")
SMTP_PORT = int(os.environ.get("SMTP_PORT", "587"))
SMTP_USER = os.environ.get("SMTP_USER", "")
SMTP_PASS = os.environ.get("SMTP_PASS", "")

def init_db():
    """रेवेन्यू टेबल बनाएं अगर नहीं है।"""
    conn = sqlite3.connect(DB_PATH)
    conn.execute("""
        CREATE TABLE IF NOT EXISTS transactions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            date TEXT NOT NULL,
            stream TEXT NOT NULL,
            type TEXT NOT NULL CHECK(type IN ('income', 'expense')),
            description TEXT,
            amount REAL NOT NULL
        )
    """)
    conn.commit()
    return conn

def generate_report(conn: sqlite3.Connection) -> str:
    """प्लेन-टेक्स्ट वीकली रिपोर्ट जनरेट करें।"""
    today = datetime.now()
    week_ago = today - timedelta(days=7)

    cursor = conn.execute("""
        SELECT stream, type, SUM(amount) as total
        FROM transactions
        WHERE date >= ? AND date <= ?
        GROUP BY stream, type
        ORDER BY stream, type
    """, (week_ago.strftime("%Y-%m-%d"), today.strftime("%Y-%m-%d")))

    rows = cursor.fetchall()

    total_income = 0
    total_expenses = 0
    streams = {}

    for stream, txn_type, amount in rows:
        if stream not in streams:
            streams[stream] = {"income": 0, "expense": 0}
        streams[stream][txn_type] = amount
        if txn_type == "income":
            total_income += amount
        else:
            total_expenses += amount

    report = []
    report.append(f"वीकली रेवेन्यू रिपोर्ट")
    report.append(f"अवधि: {week_ago.strftime('%Y-%m-%d')} से {today.strftime('%Y-%m-%d')}")
    report.append(f"जनरेट: {today.strftime('%Y-%m-%d %H:%M')}")
    report.append("=" * 50)
    report.append("")

    for stream, data in sorted(streams.items()):
        net = data["income"] - data["expense"]
        report.append(f"  {stream}")
        report.append(f"    इनकम:    ${data['income']:>10,.2f}")
        report.append(f"    खर्चे:    ${data['expense']:>10,.2f}")
        report.append(f"    नेट:      ${net:>10,.2f}")
        report.append("")

    report.append("=" * 50)
    report.append(f"  कुल इनकम:    ${total_income:>10,.2f}")
    report.append(f"  कुल खर्चे:    ${total_expenses:>10,.2f}")
    report.append(f"  नेट प्रॉफिट:  ${total_income - total_expenses:>10,.2f}")

    if total_expenses > 0:
        roi = (total_income - total_expenses) / total_expenses
        report.append(f"  ROI:          {roi:>10.1f}x")

    return "\n".join(report)

def send_email(subject: str, body: str):
    """ईमेल से रिपोर्ट भेजें।"""
    msg = MIMEText(body, "plain")
    msg["Subject"] = subject
    msg["From"] = SMTP_USER
    msg["To"] = EMAIL_TO

    with smtplib.SMTP(SMTP_HOST, SMTP_PORT) as server:
        server.starttls()
        server.login(SMTP_USER, SMTP_PASS)
        server.sendmail(SMTP_USER, EMAIL_TO, msg.as_string())

def main():
    os.makedirs(os.path.dirname(DB_PATH), exist_ok=True)
    conn = init_db()
    report = generate_report(conn)
    print(report)

    if SMTP_USER:
        send_email(
            f"वीकली रेवेन्यू रिपोर्ट — {datetime.now().strftime('%Y-%m-%d')}",
            report
        )
        print("\nरिपोर्ट ईमेल हो गई।")
    conn.close()

if __name__ == "__main__":
    main()
```

#### ऑटोमेशन 5: क्लाइंट डेटा ऑटो-बैकअप

क्लाइंट डिलिवरेबल्स कभी न खोएं। यह हर रात चलता है और 30 दिनों के बैकअप रखता है।

```bash
#!/bin/bash
# backup_client_data.sh — क्लाइंट प्रोजेक्ट डेटा का नाइटली बैकअप।
# Cron: 0 3 * * * /home/youruser/scripts/backup_client_data.sh

BACKUP_DIR="$HOME/income/backups"
SOURCE_DIR="$HOME/income/projects"
DATE=$(date +%Y-%m-%d)
RETENTION_DAYS=30

mkdir -p "$BACKUP_DIR"

# कंप्रेस्ड बैकअप बनाएं
tar -czf "$BACKUP_DIR/projects-$DATE.tar.gz" \
    -C "$SOURCE_DIR" . \
    --exclude='node_modules' \
    --exclude='.git' \
    --exclude='target' \
    --exclude='__pycache__'

# रिटेंशन पीरियड से पुराने बैकअप डिलीट करें
find "$BACKUP_DIR" -name "projects-*.tar.gz" -mtime +"$RETENTION_DAYS" -delete

# लॉग
BACKUP_SIZE=$(du -h "$BACKUP_DIR/projects-$DATE.tar.gz" | cut -f1)
echo "$(date -Iseconds) बैकअप पूरा: $BACKUP_SIZE" >> "$HOME/income/logs/backup.log"

# वैकल्पिक: दूसरे लोकेशन पर सिंक (एक्सटर्नल ड्राइव, दूसरी मशीन)
# rsync -a "$BACKUP_DIR/projects-$DATE.tar.gz" /mnt/external/backups/
```

### Systemd टाइमर अधिक कंट्रोल के लिए

अगर आपको cron से ज्यादा चाहिए — जैसे डिपेंडेंसी ऑर्डरिंग, रिसोर्स लिमिट, या ऑटोमैटिक रिट्राई — systemd टाइमर उपयोग करें:

```ini
# /etc/systemd/system/income-publisher.service
[Unit]
Description=Publish scheduled content
After=network-online.target
Wants=network-online.target

[Service]
Type=oneshot
User=youruser
ExecStart=/usr/bin/python3 /home/youruser/scripts/scheduled_publisher.py
Environment="CMS_API_KEY=your-key-here"
Environment="CMS_API_URL=https://api.example.com/posts"
# फेलियर पर एक्सपोनेंशियल बैकऑफ के साथ रीस्टार्ट
Restart=on-failure
RestartSec=60

[Install]
WantedBy=multi-user.target
```

```ini
# /etc/systemd/system/income-publisher.timer
[Unit]
Description=Run content publisher daily at 6 AM

[Timer]
OnCalendar=*-*-* 06:00:00
Persistent=true
# अगर मशीन सुबह 6 बजे बंद थी, ऑनलाइन आने पर चलाएं
RandomizedDelaySec=300

[Install]
WantedBy=timers.target
```

```bash
# टाइमर इनेबल और स्टार्ट करें
sudo systemctl enable income-publisher.timer
sudo systemctl start income-publisher.timer

# स्टेटस चेक करें
systemctl list-timers --all | grep income

# लॉग देखें
journalctl -u income-publisher.service --since today
```

{? if computed.os_family == "windows" ?}
### Windows Task Scheduler विकल्प

अगर आप WSL नहीं उपयोग कर रहे हैं, Windows Task Scheduler वही काम करता है। कमांड लाइन से `schtasks` या Task Scheduler GUI (`taskschd.msc`) उपयोग करें। मुख्य अंतर: cron एक सिंगल एक्सप्रेशन उपयोग करता है, Task Scheduler ट्रिगर, एक्शन और कंडीशन के लिए अलग-अलग फील्ड उपयोग करता है। इस पाठ का हर cron उदाहरण सीधे ट्रांसलेट होता है — अपनी Python स्क्रिप्ट उसी तरह शेड्यूल करें, बस एक अलग इंटरफेस से।
{? endif ?}

### आपकी बारी

1. इस पाठ से सबसे सरल ऑटोमेशन चुनें जो आपकी इनकम स्ट्रीम पर लागू हो।
2. इसे इम्प्लीमेंट करें। "इम्प्लीमेंट करने की प्लान बनाएं" नहीं। कोड लिखें, टेस्ट करें, शेड्यूल करें।
3. लॉगिंग सेटअप करें ताकि आप वेरिफाई कर सकें कि यह चल रही है। 3 दिन तक हर सुबह लॉग चेक करें।
4. एक बार स्टेबल हो जाए, डेली चेक बंद करें। वीकली चेक करें। यही ऑटोमेशन है।

**न्यूनतम:** आज के अंत तक एक cron जॉब विश्वसनीय रूप से चल रही हो।

---

## पाठ 3: लेवल 2 से 3 — LLM-पावर्ड पाइपलाइन

*"अपनी ऑटोमेशन में इंटेलिजेंस जोड़ें। यहां एक व्यक्ति टीम जैसा दिखने लगता है।"*

### पैटर्न

हर LLM-पावर्ड पाइपलाइन एक ही शेप फॉलो करती है:

```
इनपुट सोर्स → इंजेस्ट → LLM प्रोसेस → आउटपुट फॉर्मेट → डिलीवर (या रिव्यू के लिए क्यू)
```

जादू "LLM प्रोसेस" स्टेप में है। हर संभव केस के लिए डिटर्मिनिस्टिक रूल लिखने के बजाय, आप नेचुरल लैंग्वेज में बताते हैं कि क्या चाहिए, और LLM जज्मेंट कॉल्स संभालता है।

### लोकल vs API कब उपयोग करें

{? if settings.has_llm ?}
आपके पास {= settings.llm_provider | fallback("एक LLM प्रोवाइडर") =} कॉन्फ़िगर है {= settings.llm_model | fallback("आपके LLM मॉडल") =} के साथ। इसका मतलब आप तुरंत इंटेलिजेंट पाइपलाइन बनाना शुरू कर सकते हैं। नीचे का निर्णय आपको हर पाइपलाइन के लिए लोकल सेटअप vs API के बीच चुनने में मदद करता है।
{? else ?}
आपने अभी तक LLM कॉन्फ़िगर नहीं किया है। इस पाठ की पाइपलाइन लोकल मॉडल (Ollama) और क्लाउड API दोनों से काम करती हैं। अपनी पहली पाइपलाइन बनाने से पहले कम से कम एक सेटअप करें — Ollama मुफ्त है और इंस्टॉल में 10 मिनट लगते हैं।
{? endif ?}

इस निर्णय का आपकी मार्जिन पर सीधा प्रभाव है:

| फैक्टर | लोकल (Ollama) | API (Claude, GPT) |
|--------|--------------|-------------------|
| **प्रति 1M टोकन लागत** | ~$0.003 (बिजली) | $0.15 - $15.00 |
| **स्पीड (टोकन/सेकंड)** | 20-60 (8B मिड-रेंज GPU पर) | 50-100+ |
| **क्वालिटी (8B लोकल vs API)** | क्लासिफिकेशन, एक्सट्रैक्शन के लिए अच्छा | जनरेशन, रीजनिंग के लिए बेहतर |
| **प्राइवेसी** | डेटा आपकी मशीन कभी नहीं छोड़ता | डेटा प्रोवाइडर को जाता है |
| **अपटाइम** | आपकी मशीन पर निर्भर | 99.9%+ |
| **बैच कैपेसिटी** | GPU मेमोरी से सीमित | रेट लिमिट और बजट से सीमित |

{? if profile.gpu.exists ?}
आपकी मशीन पर {= profile.gpu.model | fallback("आपका GPU") =} होने से, लोकल इन्फरेंस एक मजबूत विकल्प है। आप कौन सी स्पीड और मॉडल साइज चला सकते हैं यह आपके VRAM पर निर्भर करता है — लोकल-ओनली पाइपलाइन के लिए कमिट करने से पहले चेक करें कि क्या फिट होता है।
{? if computed.has_nvidia ?}
NVIDIA GPU को CUDA एक्सेलरेशन की बदौलत Ollama के साथ बेस्ट परफॉर्मेंस मिलती है। आप 7-8B पैरामीटर मॉडल आराम से चला सकते हैं, और शायद बड़े भी आपके {= profile.gpu.vram | fallback("उपलब्ध VRAM") =} के आधार पर।
{? endif ?}
{? else ?}
डेडिकेटेड GPU के बिना, लोकल इन्फरेंस धीमा होगा (सिर्फ CPU)। यह छोटे बैच जॉब और क्लासिफिकेशन टास्क के लिए अभी भी काम करता है, लेकिन टाइम-सेंसिटिव या हाई-वॉल्यूम किसी भी चीज के लिए, API मॉडल अधिक प्रैक्टिकल होगा।
{? endif ?}

**सामान्य नियम:**
- **हाई वॉल्यूम, कम क्वालिटी बार** (क्लासिफिकेशन, एक्सट्रैक्शन, टैगिंग) → लोकल
- **लो वॉल्यूम, क्वालिटी-क्रिटिकल** (कस्टमर-फेसिंग कंटेंट, कॉम्प्लेक्स एनालिसिस) → API
- **सेंसिटिव डेटा** (क्लाइंट इंफो, प्रोप्राइटरी डेटा) → लोकल, हमेशा
- **10,000 से अधिक आइटम/माह** → लोकल असली पैसा बचाता है

**एक टिपिकल पाइपलाइन के लिए मंथली कॉस्ट कम्पैरिजन:**

```
5,000 आइटम/माह प्रोसेस करना, ~500 टोकन प्रति आइटम:

लोकल (Ollama, llama3.1:8b):
  2,500,000 टोकन × $0.003/1M = $0.0075/माह
  प्रैक्टिकली मुफ्त।

API (GPT-4o-mini):
  2,500,000 इनपुट टोकन × $0.15/1M = $0.375
  2,500,000 आउटपुट टोकन × $0.60/1M = $1.50
  कुल: ~$1.88/माह
  सस्ता, लेकिन लोकल से 250 गुना।

API (Claude 3.5 Sonnet):
  2,500,000 इनपुट टोकन × $3.00/1M = $7.50
  2,500,000 आउटपुट टोकन × $15.00/1M = $37.50
  कुल: ~$45/माह
  क्वालिटी बढ़िया है, लेकिन लोकल से 6,000 गुना।
```

क्लासिफिकेशन और एक्सट्रैक्शन पाइपलाइन के लिए, अच्छे प्रॉम्प्ट वाले 8B लोकल मॉडल और फ्रंटियर API मॉडल के बीच क्वालिटी का अंतर अक्सर नगण्य होता है। दोनों टेस्ट करें। जो सस्ता आपकी क्वालिटी बार पूरी करे वो उपयोग करें।

{@ insight cost_projection @}

### पाइपलाइन 1: न्यूजलेटर कंटेंट जनरेटर

कंटेंट-बेस्ड इनकम वाले डेवलपर्स के लिए यह सबसे आम LLM ऑटोमेशन है। RSS फीड अंदर जाती हैं, ड्राफ्ट न्यूजलेटर बाहर आता है।

```python
#!/usr/bin/env python3
"""
newsletter_pipeline.py — RSS फीड इंजेस्ट करें, LLM से सारांशित करें, न्यूजलेटर ड्राफ्ट जनरेट करें।
डेली चलाएं: 0 5 * * * python3 /path/to/newsletter_pipeline.py

यह पाइपलाइन:
1. कई RSS फीड से नए आर्टिकल फेच करती है
2. हर एक को लोकल LLM को सारांश के लिए भेजती है
3. आपके ऑडियंस के लिए रेलेवेंस के अनुसार रैंक करती है
4. फॉर्मेटेड न्यूजलेटर ड्राफ्ट जनरेट करती है
5. आपकी रिव्यू के लिए ड्राफ्ट सेव करती है (आप 10 मिनट रिव्यू करते हैं, 2 घंटे क्यूरेट नहीं)
"""

import os
import json
import hashlib
import requests
import xml.etree.ElementTree as ET
from datetime import datetime, timedelta
from pathlib import Path

OLLAMA_URL = "http://127.0.0.1:11434/api/generate"
MODEL = "llama3.1:8b"

FEEDS = [
    "https://hnrss.org/frontpage",
    "https://blog.rust-lang.org/feed.xml",
    "https://this-week-in-rust.org/atom.xml",
    # अपने निश फीड यहां जोड़ें
]

SEEN_FILE = os.path.expanduser("~/income/newsletter/seen.json")
DRAFTS_DIR = os.path.expanduser("~/income/newsletter/drafts")
AUDIENCE_DESCRIPTION = "Rust developers interested in systems programming, AI/ML, and developer tooling"

def load_seen() -> set:
    try:
        with open(SEEN_FILE, "r") as f:
            return set(json.load(f))
    except (FileNotFoundError, json.JSONDecodeError):
        return set()

def save_seen(seen: set):
    os.makedirs(os.path.dirname(SEEN_FILE), exist_ok=True)
    with open(SEEN_FILE, "w") as f:
        json.dump(sorted(seen), f)

def fetch_feed(url: str) -> list:
    """RSS/Atom फीड पार्स करें और आर्टिकल रिटर्न करें।"""
    try:
        resp = requests.get(url, timeout=30, headers={
            "User-Agent": "NewsletterBot/1.0"
        })
        resp.raise_for_status()
        root = ET.fromstring(resp.content)

        articles = []
        # RSS और Atom दोनों फीड हैंडल करें
        for item in root.findall(".//{http://www.w3.org/2005/Atom}entry") or root.findall(".//item"):
            title = (item.findtext("{http://www.w3.org/2005/Atom}title")
                     or item.findtext("title") or "")
            link = (item.find("{http://www.w3.org/2005/Atom}link")
                    or item.find("link"))
            if link is not None:
                link_url = link.get("href", "") or link.text or ""
            else:
                link_url = ""

            description = (item.findtext("{http://www.w3.org/2005/Atom}summary")
                           or item.findtext("description") or "")

            article_id = hashlib.md5(f"{title}{link_url}".encode()).hexdigest()

            articles.append({
                "id": article_id,
                "title": title.strip(),
                "link": link_url.strip(),
                "description": description[:500].strip(),
                "source": url
            })
        return articles
    except Exception as e:
        print(f"  {url} फेच विफल: {e}")
        return []

def llm_process(prompt: str) -> str:
    """लोकल LLM को प्रॉम्प्ट भेजें और रिस्पॉन्स पाएं।"""
    payload = {
        "model": MODEL,
        "prompt": prompt,
        "stream": False,
        "options": {
            "temperature": 0.3,
            "num_ctx": 4096
        }
    }

    try:
        resp = requests.post(OLLAMA_URL, json=payload, timeout=120)
        resp.raise_for_status()
        return resp.json().get("response", "").strip()
    except Exception as e:
        print(f"  LLM एरर: {e}")
        return ""

def score_and_summarize(article: dict) -> dict:
    """LLM का उपयोग करके रेलेवेंस स्कोर करें और सारांश बनाएं।"""
    prompt = f"""You are a newsletter curator for an audience of: {AUDIENCE_DESCRIPTION}

Article title: {article['title']}
Article excerpt: {article['description']}

Respond in this exact JSON format (no other text):
{{
  "relevance": <1-10 integer, 10 = extremely relevant to the audience>,
  "summary": "<2-3 sentence summary focusing on why this matters to the audience>",
  "category": "<one of: tool, technique, news, opinion, tutorial>"
}}"""

    result_text = llm_process(prompt)

    try:
        cleaned = result_text.strip()
        if cleaned.startswith("```"):
            cleaned = cleaned.split("\n", 1)[1].rsplit("```", 1)[0]
        result = json.loads(cleaned)
        article["relevance"] = result.get("relevance", 5)
        article["summary"] = result.get("summary", article["description"][:200])
        article["category"] = result.get("category", "news")
    except (json.JSONDecodeError, KeyError):
        article["relevance"] = 5
        article["summary"] = article["description"][:200]
        article["category"] = "news"

    return article

def generate_newsletter(articles: list) -> str:
    """स्कोर किए आर्टिकल को न्यूजलेटर ड्राफ्ट में फॉर्मेट करें।"""
    today = datetime.now().strftime("%Y-%m-%d")

    sections = {"tool": [], "technique": [], "news": [], "opinion": [], "tutorial": []}
    for article in articles:
        cat = article.get("category", "news")
        if cat in sections:
            sections[cat].append(article)

    newsletter = []
    newsletter.append(f"# आपका न्यूजलेटर — {today}")
    newsletter.append("")
    newsletter.append("*[आपका इंट्रो यहां — इस हफ्ते की थीम के बारे में 2-3 वाक्य लिखें]*")
    newsletter.append("")

    section_titles = {
        "tool": "टूल्स और रिलीज",
        "technique": "टेक्नीक्स और पैटर्न",
        "news": "इंडस्ट्री न्यूज",
        "tutorial": "ट्यूटोरियल और गाइड",
        "opinion": "दृष्टिकोण"
    }

    for cat, title in section_titles.items():
        items = sections.get(cat, [])
        if not items:
            continue

        newsletter.append(f"## {title}")
        newsletter.append("")

        for item in items:
            newsletter.append(f"**[{item['title']}]({item['link']})**")
            newsletter.append(f"{item['summary']}")
            newsletter.append("")

    newsletter.append("---")
    newsletter.append("*[आपकी क्लोजिंग — आप किस पर काम कर रहे हैं? रीडर्स को किस पर नज़र रखनी चाहिए?]*")

    return "\n".join(newsletter)

def main():
    seen = load_seen()
    all_articles = []

    print("फीड फेच हो रहे हैं...")
    for feed_url in FEEDS:
        articles = fetch_feed(feed_url)
        new_articles = [a for a in articles if a["id"] not in seen]
        all_articles.extend(new_articles)
        print(f"  {feed_url}: {len(new_articles)} नए आर्टिकल")

    if not all_articles:
        print("कोई नया आर्टिकल नहीं। स्किप।")
        return

    print(f"\nLLM से {len(all_articles)} आर्टिकल स्कोर हो रहे हैं...")
    scored = []
    for i, article in enumerate(all_articles):
        print(f"  [{i+1}/{len(all_articles)}] {article['title'][:60]}...")
        scored_article = score_and_summarize(article)
        scored.append(scored_article)
        seen.add(article["id"])

    # सिर्फ रेलेवेंट आर्टिकल फिल्टर करें और स्कोर से सॉर्ट करें
    relevant = [a for a in scored if a.get("relevance", 0) >= 6]
    relevant.sort(key=lambda x: x.get("relevance", 0), reverse=True)

    # टॉप 10 लें
    top_articles = relevant[:10]

    print(f"\n{len(top_articles)} आर्टिकल ने रेलेवेंस थ्रेशोल्ड पास किया (>= 6/10)")

    # न्यूजलेटर ड्राफ्ट जनरेट करें
    draft = generate_newsletter(top_articles)

    # ड्राफ्ट सेव करें
    os.makedirs(DRAFTS_DIR, exist_ok=True)
    draft_path = os.path.join(DRAFTS_DIR, f"draft-{datetime.now().strftime('%Y-%m-%d')}.md")
    with open(draft_path, "w", encoding="utf-8") as f:
        f.write(draft)

    save_seen(seen)
    print(f"\nड्राफ्ट सेव हुआ: {draft_path}")
    print("रिव्यू करें, अपना इंट्रो/क्लोजिंग जोड़ें, और भेजें।")

if __name__ == "__main__":
    main()
```

**इसकी लागत:**
- लोकल 8B मॉडल से 50 आर्टिकल/दिन प्रोसेस: ~$0/माह
- आपका समय: ड्राफ्ट रिव्यू करने में 10 मिनट vs मैन्युअली क्यूरेट करने में 2 घंटे
- प्रति सप्ताह बचा समय: ~10 घंटे अगर आप वीकली न्यूजलेटर चलाते हैं

### पाइपलाइन 2: कस्टमर रिसर्च और इनसाइट रिपोर्ट

यह पाइपलाइन पब्लिक डेटा स्क्रैप करती है, LLM से एनालाइज करती है, और एक रिपोर्ट प्रोड्यूस करती है जो आप बेच सकते हैं।

```python
#!/usr/bin/env python3
"""
research_pipeline.py — पब्लिक कंपनी/प्रोडक्ट डेटा एनालाइज करें और इनसाइट रिपोर्ट जनरेट करें।
यह एक सर्विस है जो आप बेच सकते हैं: $200-500 प्रति कस्टम रिपोर्ट।

उपयोग: python3 research_pipeline.py "Company Name" "their-website.com"
"""

import os
import sys
import json
import requests
from datetime import datetime

OLLAMA_URL = "http://127.0.0.1:11434/api/generate"
# पेड रिपोर्ट पर क्वालिटी के लिए बड़ा मॉडल उपयोग करें
MODEL = os.environ.get("RESEARCH_MODEL", "llama3.1:8b")
# या कस्टमर-फेसिंग क्वालिटी के लिए API:
ANTHROPIC_KEY = os.environ.get("ANTHROPIC_API_KEY", "")
USE_API = bool(ANTHROPIC_KEY)

REPORTS_DIR = os.path.expanduser("~/income/reports")

def llm_query(prompt: str, max_tokens: int = 2000) -> str:
    """कॉन्फ़िगरेशन के आधार पर लोकल या API मॉडल को रूट करें।"""
    if USE_API:
        return llm_query_api(prompt, max_tokens)
    return llm_query_local(prompt, max_tokens)

def llm_query_local(prompt: str, max_tokens: int = 2000) -> str:
    resp = requests.post(OLLAMA_URL, json={
        "model": MODEL,
        "prompt": prompt,
        "stream": False,
        "options": {"temperature": 0.4, "num_ctx": 8192}
    }, timeout=180)
    resp.raise_for_status()
    return resp.json().get("response", "")

def llm_query_api(prompt: str, max_tokens: int = 2000) -> str:
    resp = requests.post(
        "https://api.anthropic.com/v1/messages",
        headers={
            "x-api-key": ANTHROPIC_KEY,
            "anthropic-version": "2023-06-01",
            "content-type": "application/json"
        },
        json={
            "model": "claude-sonnet-4-20250514",
            "max_tokens": max_tokens,
            "messages": [{"role": "user", "content": prompt}]
        },
        timeout=120
    )
    resp.raise_for_status()
    return resp.json()["content"][0]["text"]

def gather_public_data(company: str, domain: str) -> dict:
    """किसी कंपनी के बारे में पब्लिकली उपलब्ध डेटा इकट्ठा करें।"""
    data = {"company": company, "domain": domain}

    try:
        resp = requests.get(
            f"https://{domain}",
            timeout=15,
            headers={"User-Agent": "Mozilla/5.0 (ResearchBot/1.0)"},
            allow_redirects=True
        )
        data["website_status"] = resp.status_code
        data["website_title"] = ""
        if "<title>" in resp.text.lower():
            start = resp.text.lower().index("<title>") + 7
            end = resp.text.lower().index("</title>")
            data["website_title"] = resp.text[start:end].strip()
    except Exception as e:
        data["website_status"] = f"Error: {e}"

    try:
        gh_resp = requests.get(
            f"https://api.github.com/orgs/{company.lower().replace(' ', '-')}",
            timeout=10,
            headers={"Accept": "application/vnd.github.v3+json"}
        )
        if gh_resp.status_code == 200:
            gh_data = gh_resp.json()
            data["github_repos"] = gh_data.get("public_repos", 0)
            data["github_followers"] = gh_data.get("followers", 0)
    except Exception:
        pass

    return data

def generate_report(company: str, domain: str, data: dict) -> str:
    """LLM का उपयोग करके एनालिसिस रिपोर्ट जनरेट करें।"""
    context = json.dumps(data, indent=2)

    analysis_prompt = f"""You are a technology market analyst. Generate a concise research report about {company} ({domain}).

Available data:
{context}

Generate a report with these sections:
1. Company Overview (2-3 sentences based on available data)
2. Technical Stack Assessment (what can be inferred from their public presence)
3. Market Position (based on GitHub activity, web presence)
4. Opportunities (what services or products could someone offer TO this company)
5. Risks (any red flags for doing business with them)

Keep each section to 3-5 bullet points. Be specific and data-driven.
Format as clean markdown."""

    return llm_query(analysis_prompt, max_tokens=2000)

def main():
    if len(sys.argv) < 3:
        print("उपयोग: python3 research_pipeline.py 'Company Name' 'domain.com'")
        sys.exit(1)

    company = sys.argv[1]
    domain = sys.argv[2]

    print(f"रिसर्च: {company} ({domain})")
    print(f"उपयोग: {'API (Claude)' if USE_API else 'Local (Ollama)'}")

    print("पब्लिक डेटा इकट्ठा हो रहा है...")
    data = gather_public_data(company, domain)

    print("एनालिसिस जनरेट हो रहा है...")
    report = generate_report(company, domain, data)

    final_report = f"""# रिसर्च रिपोर्ट: {company}

**जनरेट:** {datetime.now().strftime('%Y-%m-%d %H:%M')}
**डोमेन:** {domain}
**एनालिसिस मॉडल:** {'Claude Sonnet' if USE_API else MODEL}

---

{report}

---

*यह रिपोर्ट केवल पब्लिकली उपलब्ध डेटा का उपयोग करके जनरेट की गई है।
किसी प्रोप्राइटरी या प्राइवेट डेटा तक एक्सेस नहीं किया गया।*
"""

    os.makedirs(REPORTS_DIR, exist_ok=True)
    filename = f"{company.lower().replace(' ', '-')}-{datetime.now().strftime('%Y%m%d')}.md"
    filepath = os.path.join(REPORTS_DIR, filename)

    with open(filepath, "w", encoding="utf-8") as f:
        f.write(final_report)

    print(f"\nरिपोर्ट सेव हुई: {filepath}")
    print(f"API लागत: ~${'0.02-0.05' if USE_API else '0.00'}")

if __name__ == "__main__":
    main()
```

**बिजनेस मॉडल:** $200-500 प्रति कस्टम रिसर्च रिपोर्ट चार्ज करें। आपकी लागत: API कॉल में $0.05 और 15 मिनट रिव्यू। पाइपलाइन स्टेबल होने पर आप प्रति घंटा 3-4 रिपोर्ट प्रोड्यूस कर सकते हैं।

### पाइपलाइन 3: मार्केट सिग्नल मॉनिटर

यह वह पाइपलाइन है जो आपको बताती है कि आगे क्या बनाना है। यह कई सोर्स मॉनिटर करती है, सिग्नल क्लासिफाई करती है, और जब कोई अवसर आपकी थ्रेशोल्ड पार करे तो अलर्ट करती है।

```python
#!/usr/bin/env python3
"""
signal_monitor.py — मार्केट अवसरों के लिए पब्लिक सोर्स मॉनिटर करें।
हर 2 घंटे चलाएं: 0 */2 * * * python3 /path/to/signal_monitor.py
"""

import os
import json
import hashlib
import requests
from datetime import datetime
from pathlib import Path

OLLAMA_URL = "http://127.0.0.1:11434/api/generate"
MODEL = "llama3.1:8b"

DATA_DIR = os.path.expanduser("~/income/signals")
ALERTS_FILE = os.path.join(DATA_DIR, "alerts.jsonl")
SEEN_FILE = os.path.join(DATA_DIR, "seen.json")

SLACK_WEBHOOK = os.environ.get("SLACK_WEBHOOK_URL", "")

MY_NICHE = """
I build developer tools and local-first software. I know Rust, TypeScript, and Python.
I sell digital products (templates, starter kits), consulting, and a niche newsletter.
My audience is developers interested in privacy, local AI, and desktop apps.
"""

def load_seen() -> set:
    try:
        with open(SEEN_FILE, "r") as f:
            return set(json.load(f))
    except (FileNotFoundError, json.JSONDecodeError):
        return set()

def save_seen(seen: set):
    os.makedirs(DATA_DIR, exist_ok=True)
    with open(SEEN_FILE, "w") as f:
        json.dump(sorted(seen), f)

def fetch_hn_top(limit: int = 30) -> list:
    """Hacker News की टॉप स्टोरी फेच करें।"""
    try:
        ids_resp = requests.get(
            "https://hacker-news.firebaseio.com/v0/topstories.json",
            timeout=15
        )
        ids = ids_resp.json()[:limit]

        items = []
        for item_id in ids:
            item_resp = requests.get(
                f"https://hacker-news.firebaseio.com/v0/item/{item_id}.json",
                timeout=10
            )
            data = item_resp.json()
            if data and data.get("type") == "story":
                items.append({
                    "id": f"hn-{item_id}",
                    "source": "hackernews",
                    "title": data.get("title", ""),
                    "url": data.get("url", f"https://news.ycombinator.com/item?id={item_id}"),
                    "score": data.get("score", 0),
                    "comments": data.get("descendants", 0)
                })
        return items
    except Exception as e:
        print(f"  HN फेच विफल: {e}")
        return []

def classify_signal(item: dict) -> dict:
    """LLM का उपयोग करके सिग्नल को मार्केट अवसर के लिए क्लासिफाई करें।"""
    prompt = f"""You are a market analyst helping a developer find income opportunities.

Developer profile:
{MY_NICHE}

Signal:
- Source: {item['source']}
- Title: {item['title']}
- URL: {item.get('url', 'N/A')}
- Engagement: score={item.get('score', 'N/A')}, comments={item.get('comments', 'N/A')}

Classify this signal. Respond in this exact JSON format only:
{{
  "opportunity_score": <0-10, 10 = perfect opportunity for this developer>,
  "opportunity_type": "<one of: product_gap, education_gap, market_shift, tool_need, community_demand, not_relevant>",
  "reasoning": "<one sentence explaining why this is or isn't an opportunity>",
  "action": "<specific next step if score >= 7, or 'none'>"
}}"""

    try:
        resp = requests.post(OLLAMA_URL, json={
            "model": MODEL,
            "prompt": prompt,
            "stream": False,
            "options": {"temperature": 0.2, "num_ctx": 4096}
        }, timeout=120)
        resp.raise_for_status()

        result_text = resp.json().get("response", "").strip()
        if result_text.startswith("```"):
            result_text = result_text.split("\n", 1)[1].rsplit("```", 1)[0]

        classification = json.loads(result_text)
        item.update(classification)
    except (json.JSONDecodeError, Exception) as e:
        item["opportunity_score"] = 0
        item["opportunity_type"] = "not_relevant"
        item["reasoning"] = f"क्लासिफिकेशन विफल: {e}"
        item["action"] = "none"

    return item

def alert_on_opportunity(item: dict):
    """हाई-स्कोरिंग अवसरों के लिए अलर्ट भेजें।"""
    msg = (
        f"अवसर डिटेक्ट (स्कोर: {item['opportunity_score']}/10)\n"
        f"प्रकार: {item['opportunity_type']}\n"
        f"टाइटल: {item['title']}\n"
        f"URL: {item.get('url', 'N/A')}\n"
        f"कारण: {item['reasoning']}\n"
        f"एक्शन: {item['action']}"
    )

    os.makedirs(DATA_DIR, exist_ok=True)
    with open(ALERTS_FILE, "a") as f:
        entry = {**item, "alerted_at": datetime.utcnow().isoformat() + "Z"}
        f.write(json.dumps(entry) + "\n")

    if SLACK_WEBHOOK:
        try:
            requests.post(SLACK_WEBHOOK, json={"text": msg}, timeout=10)
        except Exception:
            pass

    print(f"  अलर्ट: {msg}")

def main():
    seen = load_seen()

    print("सिग्नल फेच हो रहे हैं...")
    items = fetch_hn_top(30)

    new_items = [i for i in items if i["id"] not in seen]
    print(f"  {len(new_items)} नए सिग्नल क्लासिफाई करने हैं")

    for i, item in enumerate(new_items):
        print(f"  [{i+1}/{len(new_items)}] {item['title'][:50]}...")
        classified = classify_signal(item)
        seen.add(item["id"])

        if classified.get("opportunity_score", 0) >= 7:
            alert_on_opportunity(classified)

    save_seen(seen)
    print("पूरा।")

if __name__ == "__main__":
    main()
```

**प्रैक्टिस में यह क्या करता है:** आपको हफ्ते में 2-3 बार Slack नोटिफिकेशन मिलती है कुछ ऐसा: "अवसर: नया फ्रेमवर्क रिलीज हुआ बिना स्टार्टर किट — आप इस वीकेंड एक बना सकते हैं।" वह सिग्नल, दूसरों से पहले उस पर एक्ट करना, यही है कि आप आगे कैसे रहते हैं।

> **सीधी बात:** इन पाइपलाइन आउटपुट की क्वालिटी पूरी तरह आपके प्रॉम्प्ट और निश डेफिनिशन पर निर्भर करती है। अगर आपका निश अस्पष्ट है ("मैं वेब डेवलपर हूं"), LLM सब कुछ फ्लैग करेगा। अगर यह स्पेसिफिक है ("मैं प्राइवेसी-फर्स्ट डेवलपर मार्केट के लिए Tauri डेस्कटॉप ऐप बनाता हूं"), यह सर्जिकली प्रिसाइज होगा। अपनी निश डेफिनिशन सही करने में 30 मिनट खर्च करें। यह हर पाइपलाइन का सबसे हाई-लीवरेज इनपुट है।

### आपकी बारी

{? if stack.contains("python") ?}
अच्छी खबर: ऊपर के पाइपलाइन उदाहरण पहले से आपकी प्राइमरी लैंग्वेज में हैं। आप उन्हें सीधे कॉपी कर सकते हैं और अडैप्ट करना शुरू कर सकते हैं। निश डेफिनिशन और प्रॉम्प्ट सही करने पर फोकस करें — आउटपुट क्वालिटी का 90% वहां से आता है।
{? else ?}
ऊपर के उदाहरण पोर्टेबिलिटी के लिए Python उपयोग करते हैं, लेकिन पैटर्न किसी भी भाषा में काम करते हैं। अगर आप {= stack.primary | fallback("अपनी प्राइमरी स्टैक") =} में बनाना पसंद करते हैं, रेप्लिकेट करने वाले मुख्य पीस हैं: RSS/API फेचिंग के लिए HTTP क्लाइंट, LLM रिस्पॉन्स के लिए JSON पार्सिंग, और स्टेट मैनेजमेंट के लिए फाइल I/O। LLM इंटरैक्शन बस Ollama या क्लाउड API को HTTP POST है।
{? endif ?}

1. ऊपर की तीन पाइपलाइन में से एक चुनें (न्यूजलेटर, रिसर्च, या सिग्नल मॉनिटर)।
2. इसे अपने निश के लिए अडैप्ट करें। फीड, ऑडियंस डिस्क्रिप्शन, क्लासिफिकेशन क्राइटेरिया बदलें।
3. आउटपुट क्वालिटी टेस्ट करने के लिए इसे मैन्युअली 3 बार चलाएं।
4. प्रॉम्प्ट ट्यून करें जब तक आउटपुट बिना भारी एडिटिंग के उपयोगी न हो।
5. cron से शेड्यूल करें।

**लक्ष्य:** इस पाठ पढ़ने के 48 घंटे के भीतर एक LLM-पावर्ड पाइपलाइन शेड्यूल पर चल रही हो।

---

## पाठ 4: लेवल 3 से 4 — एजेंट-बेस्ड सिस्टम

*"एक एजेंट बस एक लूप है जो ऑब्जर्व, डिसाइड और एक्ट करता है। एक बनाओ।"*

### 2026 में "एजेंट" का वास्तव में क्या मतलब है

हाइप हटाइए। एजेंट एक प्रोग्राम है जो:

1. **ऑब्जर्व करता है** — कोई इनपुट या स्टेट पढ़ता है
2. **डिसाइड करता है** — LLM का उपयोग करके तय करता है क्या करना है
3. **एक्ट करता है** — निर्णय एक्सीक्यूट करता है
4. **लूप करता है** — स्टेप 1 पर वापस जाता है

बस। पाइपलाइन (लेवल 3) और एजेंट (लेवल 4) के बीच अंतर यह है कि एजेंट लूप करता है। यह अपने खुद के आउटपुट पर एक्ट करता है। यह मल्टी-स्टेप टास्क हैंडल करता है जहां अगला स्टेप पिछले के रिजल्ट पर निर्भर करता है।

पाइपलाइन आइटम को एक-एक करके फिक्स्ड सीक्वेंस से प्रोसेस करती है। एजेंट जो एनकाउंटर करता है उसके आधार पर अनप्रेडिक्टेबल सीक्वेंस नेविगेट करता है।

### MCP सर्वर जो कस्टमर सर्व करते हैं

MCP सर्वर सबसे प्रैक्टिकल एजेंट-एडजेसेंट सिस्टम में से एक है जो आप बना सकते हैं। यह ऐसे टूल्स एक्सपोज करता है जो AI एजेंट (Claude Code, Cursor, आदि) आपके कस्टमर की ओर से कॉल कर सकता है।

{? if stack.contains("typescript") ?}
नीचे MCP सर्वर उदाहरण TypeScript उपयोग करता है — आपकी एक्सपर्टीज में। आप इसे अपनी मौजूदा TypeScript टूलिंग से एक्सटेंड कर सकते हैं और अपनी अन्य Node.js सर्विसेज के साथ डिप्लॉय कर सकते हैं।
{? endif ?}

यहां एक असली उदाहरण है: एक MCP सर्वर जो आपके प्रोडक्ट के डॉक्यूमेंटेशन से कस्टमर प्रश्नों का जवाब देता है।

```typescript
// mcp-docs-server/src/index.ts
// एक MCP सर्वर जो आपके डॉक्यूमेंटेशन से प्रश्नों का जवाब देता है।
// आपके कस्टमर अपने Claude Code को इस सर्वर पर पॉइंट करते हैं और इंस्टेंट जवाब पाते हैं।

import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { z } from "zod";
import * as fs from "fs";
import * as path from "path";

// स्टार्टअप पर अपने डॉक्स मेमोरी में लोड करें
const DOCS_DIR = process.env.DOCS_DIR || "./docs";

interface DocChunk {
  file: string;
  section: string;
  content: string;
}

function loadDocs(): DocChunk[] {
  const chunks: DocChunk[] = [];
  const files = fs.readdirSync(DOCS_DIR, { recursive: true }) as string[];

  for (const file of files) {
    if (!file.endsWith(".md")) continue;

    const fullPath = path.join(DOCS_DIR, file);
    const content = fs.readFileSync(fullPath, "utf-8");

    // बेहतर सर्च के लिए हेडिंग से स्प्लिट करें
    const sections = content.split(/^## /m);
    for (const section of sections) {
      if (section.trim().length < 20) continue;
      const firstLine = section.split("\n")[0].trim();
      chunks.push({
        file: file,
        section: firstLine,
        content: section.trim().slice(0, 2000),
      });
    }
  }

  return chunks;
}

function searchDocs(query: string, docs: DocChunk[], limit: number = 5): DocChunk[] {
  const queryWords = query.toLowerCase().split(/\s+/);

  const scored = docs.map((chunk) => {
    const text = `${chunk.section} ${chunk.content}`.toLowerCase();
    let score = 0;
    for (const word of queryWords) {
      if (text.includes(word)) score++;
      if (chunk.section.toLowerCase().includes(word)) score += 2;
    }
    return { chunk, score };
  });

  return scored
    .filter((s) => s.score > 0)
    .sort((a, b) => b.score - a.score)
    .slice(0, limit)
    .map((s) => s.chunk);
}

const docs = loadDocs();
console.error(`${DOCS_DIR} से ${docs.length} डॉक चंक लोड हुए`);

const server = new McpServer({
  name: "product-docs",
  version: "1.0.0",
});

server.tool(
  "search_docs",
  "Search the product documentation for information about a topic",
  {
    query: z.string().describe("The question or topic to search for"),
    max_results: z.number().optional().default(5).describe("Maximum results to return"),
  },
  async ({ query, max_results }) => {
    const results = searchDocs(query, docs, max_results);

    if (results.length === 0) {
      return {
        content: [
          {
            type: "text" as const,
            text: `No documentation found for: "${query}". Try different keywords or check the docs at https://yourdomain.com/docs`,
          },
        ],
      };
    }

    const formatted = results
      .map(
        (r, i) =>
          `### Result ${i + 1}: ${r.section}\n**File:** ${r.file}\n\n${r.content}`
      )
      .join("\n\n---\n\n");

    return {
      content: [
        {
          type: "text" as const,
          text: `Found ${results.length} relevant sections:\n\n${formatted}`,
        },
      ],
    };
  }
);

server.tool(
  "list_topics",
  "List all available documentation topics",
  {},
  async () => {
    const topics = [...new Set(docs.map((d) => d.section))].sort();
    return {
      content: [
        {
          type: "text" as const,
          text: `Available documentation topics:\n\n${topics.map((t) => `- ${t}`).join("\n")}`,
        },
      ],
    };
  }
);

const transport = new StdioServerTransport();
server.connect(transport);
```

```json
// mcp-docs-server/package.json
{
  "name": "product-docs-mcp",
  "version": "1.0.0",
  "type": "module",
  "scripts": {
    "build": "tsc",
    "start": "node dist/index.js"
  },
  "dependencies": {
    "@modelcontextprotocol/sdk": "^1.0.0",
    "zod": "^3.22.0"
  },
  "devDependencies": {
    "typescript": "^5.3.0",
    "@types/node": "^20.0.0"
  }
}
```

**बिजनेस मॉडल:** यह MCP सर्वर अपने कस्टमर को अपने प्रोडक्ट के हिस्से के रूप में दें। उन्हें सपोर्ट टिकट दायर किए बिना इंस्टेंट जवाब मिलते हैं। आप सपोर्ट पर कम समय बिताते हैं। सबको फायदा।

प्रीमियम के लिए: वेक्टर सर्च, वर्जन्ड डॉक्स, और कस्टमर क्या पूछ रहे हैं इसकी एनालिटिक्स के साथ होस्टेड वर्जन के लिए $9-29/माह चार्ज करें।

### ऑटोमेटेड कस्टमर फीडबैक प्रोसेसिंग

यह एजेंट कस्टमर फीडबैक (ईमेल, सपोर्ट टिकट, या फॉर्म से) पढ़ता है, क्लासिफाई करता है, और ड्राफ्ट रिस्पॉन्स और फीचर टिकट बनाता है।

```python
#!/usr/bin/env python3
"""
feedback_agent.py — कस्टमर फीडबैक को क्लासिफाइड, एक्शनेबल आइटम में प्रोसेस करें।
"AI ड्राफ्ट, ह्यूमन अप्रूव" पैटर्न।

हर घंटे चलाएं: 0 * * * * python3 /path/to/feedback_agent.py
"""

import os
import json
import requests
from datetime import datetime
from pathlib import Path

OLLAMA_URL = "http://127.0.0.1:11434/api/generate"
MODEL = "llama3.1:8b"

INBOX_DIR = os.path.expanduser("~/income/feedback/inbox")
PROCESSED_DIR = os.path.expanduser("~/income/feedback/processed")
REVIEW_DIR = os.path.expanduser("~/income/feedback/review")

def llm(prompt: str) -> str:
    resp = requests.post(OLLAMA_URL, json={
        "model": MODEL,
        "prompt": prompt,
        "stream": False,
        "options": {"temperature": 0.3, "num_ctx": 4096}
    }, timeout=120)
    resp.raise_for_status()
    return resp.json().get("response", "").strip()

def process_feedback(feedback: dict) -> dict:
    """फीडबैक क्लासिफाई करें और ड्राफ्ट रिस्पॉन्स जनरेट करें।"""

    classify_prompt = f"""Classify this customer feedback and draft a response.

Customer: {feedback.get('from', 'Unknown')}
Subject: {feedback.get('subject', 'No subject')}
Message: {feedback.get('body', '')}

Respond in this exact JSON format:
{{
  "category": "<bug_report | feature_request | support_question | praise | complaint | spam>",
  "urgency": "<low | medium | high | critical>",
  "sentiment": "<positive | neutral | negative | angry>",
  "summary": "<one sentence summary of the feedback>",
  "draft_response": "<professional, helpful draft response (2-4 sentences)>",
  "action_items": ["<list of specific actions to take>"],
  "needs_human": <true if this requires personal attention, false if draft response is sufficient>
}}"""

    result_text = llm(classify_prompt)

    try:
        cleaned = result_text.strip()
        if cleaned.startswith("```"):
            cleaned = cleaned.split("\n", 1)[1].rsplit("```", 1)[0]
        classification = json.loads(cleaned)
        feedback.update(classification)
    except (json.JSONDecodeError, Exception):
        feedback["category"] = "support_question"
        feedback["urgency"] = "medium"
        feedback["needs_human"] = True
        feedback["draft_response"] = "[क्लासिफिकेशन विफल — मैनुअल रिव्यू चाहिए]"

    feedback["processed_at"] = datetime.utcnow().isoformat() + "Z"
    return feedback

def main():
    os.makedirs(REVIEW_DIR, exist_ok=True)
    os.makedirs(PROCESSED_DIR, exist_ok=True)

    if not os.path.isdir(INBOX_DIR):
        print(f"इनबॉक्स डायरेक्टरी नहीं: {INBOX_DIR}")
        return

    inbox_files = sorted(Path(INBOX_DIR).glob("*.json"))

    if not inbox_files:
        print("कोई नया फीडबैक नहीं।")
        return

    print(f"{len(inbox_files)} फीडबैक आइटम प्रोसेस हो रहे हैं...")

    review_queue = []

    for filepath in inbox_files:
        try:
            with open(filepath, "r") as f:
                feedback = json.load(f)
        except (json.JSONDecodeError, Exception) as e:
            print(f"  स्किप {filepath.name}: {e}")
            continue

        print(f"  प्रोसेसिंग: {feedback.get('subject', 'कोई विषय नहीं')[:50]}...")
        processed = process_feedback(feedback)

        processed_path = os.path.join(PROCESSED_DIR, filepath.name)
        with open(processed_path, "w") as f:
            json.dump(processed, f, indent=2)

        review_queue.append({
            "file": filepath.name,
            "from": processed.get("from", "Unknown"),
            "category": processed.get("category", "unknown"),
            "urgency": processed.get("urgency", "medium"),
            "summary": processed.get("summary", ""),
            "needs_human": processed.get("needs_human", True),
            "draft_response": processed.get("draft_response", "")
        })

        filepath.rename(os.path.join(PROCESSED_DIR, f"original-{filepath.name}"))

    review_path = os.path.join(REVIEW_DIR, f"review-{datetime.now().strftime('%Y%m%d-%H%M')}.json")
    with open(review_path, "w") as f:
        json.dump(review_queue, f, indent=2)

    critical = sum(1 for item in review_queue if item["urgency"] == "critical")
    needs_human = sum(1 for item in review_queue if item["needs_human"])

    print(f"\nप्रोसेस हुए: {len(review_queue)}")
    print(f"क्रिटिकल: {critical}")
    print(f"आपका ध्यान चाहिए: {needs_human}")
    print(f"रिव्यू क्यू: {review_path}")

if __name__ == "__main__":
    main()
```

**प्रैक्टिस में यह कैसे काम करता है:**
1. कस्टमर फीडबैक सबमिट करते हैं (फॉर्म, ईमेल, या सपोर्ट सिस्टम से)
2. फीडबैक JSON फाइल के रूप में इनबॉक्स डायरेक्टरी में आता है
3. एजेंट हर एक प्रोसेस करता है: क्लासिफाई, सारांशित, ड्राफ्ट रिस्पॉन्स
4. आप दिन में एक या दो बार रिव्यू क्यू खोलते हैं
5. सिंपल आइटम (प्रशंसा, बेसिक सवाल अच्छे ड्राफ्ट रिस्पॉन्स के साथ) के लिए, ड्राफ्ट अप्रूव करते हैं
6. कॉम्प्लेक्स आइटम (बग, गुस्साए कस्टमर) के लिए, पर्सनल रिस्पॉन्स लिखते हैं
7. नेट समय: 2 घंटे के बजाय दिन में 15 मिनट

### AI ड्राफ्ट, ह्यूमन अप्रूव पैटर्न

यह पैटर्न प्रैक्टिकल लेवल 4 ऑटोमेशन का कोर है। एजेंट ग्रंट वर्क हैंडल करता है। आप जज्मेंट कॉल हैंडल करते हैं।

```
              ┌─────────────┐
              │ एजेंट ड्राफ्ट │
              └──────┬──────┘
                     │
              ┌──────▼──────┐
              │  रिव्यू क्यू  │
              └──────┬──────┘
                     │
          ┌──────────┼──────────┐
          │          │          │
    ┌─────▼─────┐ ┌──▼──┐ ┌────▼────┐
    │ ऑटो-सेंड │ │एडिट│ │एस्केलेट│
    │ (रूटीन)  │ │+सेंड│ │(कॉम्प्लेक्स)│
    └───────────┘ └─────┘ └─────────┘
```

**एजेंट पूरी तरह क्या हैंडल करता है vs आप क्या रिव्यू करते हैं:**

| एजेंट पूरी तरह हैंडल (बिना रिव्यू) | आप भेजने से पहले रिव्यू |
|--------------------------------------|------------------------|
| एक्नॉलेजमेंट रसीदें ("हमें आपका संदेश मिला") | गुस्साए कस्टमर को जवाब |
| स्टेटस अपडेट ("आपका रिक्वेस्ट प्रोसेस हो रहा है") | फीचर रिक्वेस्ट प्रायोरिटाइजेशन |
| FAQ रिस्पॉन्स (एग्जैक्ट मैच) | पैसे से जुड़ी कोई भी चीज (रिफंड, प्राइसिंग) |
| स्पैम क्लासिफिकेशन और डिलीशन | बग रिपोर्ट (आपको वेरिफाई करना होगा) |
| इंटरनल लॉगिंग और कैटेगोराइजेशन | कुछ भी जो पहले कभी न देखा हो |

> **सामान्य गलती:** एजेंट को पहले दिन से ऑटोनॉमसली कस्टमर को जवाब देने देना। न करें। एजेंट से सब कुछ ड्राफ्ट करवाएं, आप सब कुछ अप्रूव करें। एक हफ्ते बाद, एक्नॉलेजमेंट ऑटो-सेंड करने दें। एक महीने बाद, FAQ रिस्पॉन्स ऑटो-सेंड करने दें। इनक्रीमेंटली ट्रस्ट बनाएं — खुद के साथ और अपने कस्टमर के साथ।

### आपकी बारी

1. एक चुनें: MCP डॉक्स सर्वर बनाएं या फीडबैक प्रोसेसिंग एजेंट।
2. इसे अपने प्रोडक्ट/सर्विस के लिए अडैप्ट करें। अगर अभी कस्टमर नहीं हैं, पाठ 3 के सिग्नल मॉनिटर को अपने "कस्टमर" के रूप में उपयोग करें — इसके आउटपुट को फीडबैक एजेंट पैटर्न से प्रोसेस करें।
3. अलग-अलग इनपुट के साथ मैन्युअली 10 बार चलाएं।
4. मापें: कितने प्रतिशत आउटपुट बिना एडिटिंग के उपयोग करने लायक हैं? यह आपका ऑटोमेशन क्वालिटी स्कोर है। शेड्यूल करने से पहले 70%+ टारगेट करें।

---

## पाठ 5: ह्यूमन-इन-द-लूप सिद्धांत

*"पूर्ण ऑटोमेशन एक जाल है। आंशिक ऑटोमेशन एक सुपरपावर है।"*

### 80% ऑटोमेशन 100% को क्यों हराता है

एक स्पेसिफिक, मापने योग्य कारण है कि आपको कभी भी कस्टमर-फेसिंग प्रोसेस पूरी तरह ऑटोमेट नहीं करना चाहिए: खराब आउटपुट की लागत असममित है।

एक अच्छा ऑटोमेटेड आउटपुट आपके 5 मिनट बचाता है।
एक खराब ऑटोमेटेड आउटपुट आपको एक कस्टमर, एक पब्लिक कंप्लेंट, रिफंड, या रेपुटेशन हिट की लागत देता है जिससे रिकवर होने में महीने लगते हैं।

गणित:

```
100% ऑटोमेशन:
  1,000 आउटपुट/माह × 95% क्वालिटी = 950 अच्छे + 50 खराब
  50 खराब आउटपुट × $50 औसत लागत (रिफंड + सपोर्ट + रेपुटेशन) = $2,500/माह नुकसान

80% ऑटोमेशन + 20% ह्यूमन रिव्यू:
  800 आउटपुट ऑटो-हैंडल, 200 ह्यूमन-रिव्यू
  800 × 95% क्वालिटी = 760 अच्छे + 40 खराब ऑटो
  200 × 99% क्वालिटी = 198 अच्छे + 2 खराब ह्यूमन
  42 कुल खराब × $50 = $2,100/माह नुकसान
  लेकिन: आप 38 खराब वालों को कस्टमर तक पहुंचने से पहले पकड़ लेते हैं

  कस्टमर तक पहुंचने वाले वास्तविक खराब आउटपुट: ~4
  वास्तविक नुकसान: ~$200/माह
```

नुकसान लागत में 12 गुना कमी। 200 आउटपुट रिव्यू करने में आपका समय (शायद 2 घंटे) आपके $2,300/माह नुकसान में बचाता है।

### इन्हें कभी पूरी तरह ऑटोमेट न करें

कुछ चीजों में हमेशा ह्यूमन-इन-द-लूप होना चाहिए, चाहे AI कितना भी अच्छा हो:

1. **कस्टमर-फेसिंग कम्युनिकेशन** — एक खराब शब्दों वाला ईमेल आपको हमेशा के लिए कस्टमर गंवा सकता है। एक जेनेरिक, स्पष्ट रूप से AI-जनरेटेड रिस्पॉन्स ट्रस्ट खत्म कर सकता है। रिव्यू करें।

2. **फाइनेंशियल ट्रांजैक्शन** — रिफंड, प्राइसिंग बदलाव, इनवॉइसिंग। हमेशा रिव्यू करें। गलती की लागत असली पैसा है।

3. **आपके नाम से पब्लिश कंटेंट** — आपकी रेपुटेशन सालों में बनती है और एक खराब पोस्ट में नष्ट हो सकती है। दस मिनट की रिव्यू सस्ता बीमा है।

4. **लीगल या कम्प्लायंस-संबंधित आउटपुट** — कॉन्ट्रैक्ट, प्राइवेसी पॉलिसी, टर्म्स ऑफ सर्विस से जुड़ी कोई भी चीज। AI कॉन्फिडेंट लगने वाली लीगल गलतियां करता है।

5. **हायरिंग या लोगों के निर्णय** — अगर कभी आउटसोर्स करें, AI को किसके साथ काम करना है इसका फाइनल कॉल कभी न लेने दें।

### ऑटोमेशन डेब्ट

{@ mirror automation_risk_profile @}

ऑटोमेशन डेब्ट टेक्निकल डेब्ट से बुरा है क्योंकि यह तब तक दिखाई नहीं देता जब तक एक्सप्लोड नहीं हो जाता।

**ऑटोमेशन डेब्ट कैसा दिखता है:**
- सोशल मीडिया बॉट गलत समय पर पोस्ट करता है क्योंकि टाइमज़ोन बदल गया
- न्यूजलेटर पाइपलाइन 3 हफ्ते से ब्रोकन लिंक शामिल कर रही है क्योंकि कोई चेक नहीं करता
- प्राइस मॉनिटर तब काम करना बंद कर दिया जब कॉम्पिटिटर ने अपना पेज रीडिज़ाइन किया
- बैकअप स्क्रिप्ट चुपचाप फेल होती है क्योंकि डिस्क भर गई

**कैसे रोकें:**

```python
#!/usr/bin/env python3
"""
automation_healthcheck.py — अपनी सभी ऑटोमेशन को साइलेंट फेलियर के लिए मॉनिटर करें।
हर सुबह चलाएं: 0 7 * * * python3 /path/to/automation_healthcheck.py
"""

import os
import json
from datetime import datetime, timedelta
from pathlib import Path

ALERT_WEBHOOK = os.environ.get("SLACK_WEBHOOK_URL", "")

AUTOMATIONS = [
    {
        "name": "न्यूजलेटर पाइपलाइन",
        "check_type": "file_freshness",
        "path": os.path.expanduser("~/income/newsletter/drafts"),
        "pattern": "draft-*.md",
        "max_age_hours": 26,
    },
    {
        "name": "सोशल पोस्टर",
        "check_type": "file_freshness",
        "path": os.path.expanduser("~/income/logs/social_posted.json"),
        "pattern": None,
        "max_age_hours": 2,
    },
    {
        "name": "कॉम्पिटिटर मॉनिटर",
        "check_type": "file_freshness",
        "path": os.path.expanduser("~/income/monitors"),
        "pattern": "*.json",
        "max_age_hours": 8,
    },
    {
        "name": "क्लाइंट बैकअप",
        "check_type": "file_freshness",
        "path": os.path.expanduser("~/income/backups"),
        "pattern": "projects-*.tar.gz",
        "max_age_hours": 26,
    },
    {
        "name": "Ollama सर्वर",
        "check_type": "http",
        "url": "http://127.0.0.1:11434/api/tags",
        "expected_status": 200,
    },
]

def check_file_freshness(automation: dict) -> tuple[bool, str]:
    path = automation["path"]
    max_age = timedelta(hours=automation["max_age_hours"])

    if automation.get("pattern"):
        p = Path(path)
        if not p.exists():
            return False, f"डायरेक्टरी नहीं मिली: {path}"

        files = sorted(p.glob(automation["pattern"]), key=os.path.getmtime, reverse=True)
        if not files:
            return False, f"{path} में {automation['pattern']} से मैच करने वाली कोई फाइल नहीं"

        newest = files[0]
        age = datetime.now() - datetime.fromtimestamp(os.path.getmtime(newest))
    else:
        if not os.path.exists(path):
            return False, f"फाइल नहीं मिली: {path}"
        age = datetime.now() - datetime.fromtimestamp(os.path.getmtime(path))

    if age > max_age:
        return False, f"आखिरी आउटपुट {age.total_seconds()/3600:.1f} घंटे पहले (अधिकतम: {automation['max_age_hours']} घंटे)"

    return True, f"ठीक (आखिरी आउटपुट {age.total_seconds()/3600:.1f} घंटे पहले)"

def check_http(automation: dict) -> tuple[bool, str]:
    import requests
    try:
        resp = requests.get(automation["url"], timeout=10)
        if resp.status_code == automation.get("expected_status", 200):
            return True, f"ठीक (HTTP {resp.status_code})"
        return False, f"अनपेक्षित स्टेटस: HTTP {resp.status_code}"
    except Exception as e:
        return False, f"कनेक्शन विफल: {e}"

def send_alert(message: str):
    if ALERT_WEBHOOK:
        import requests
        requests.post(ALERT_WEBHOOK, json={"text": message}, timeout=10)
    print(message)

def main():
    failures = []

    for automation in AUTOMATIONS:
        check_type = automation["check_type"]

        if check_type == "file_freshness":
            ok, msg = check_file_freshness(automation)
        elif check_type == "http":
            ok, msg = check_http(automation)
        else:
            ok, msg = False, f"अज्ञात चेक प्रकार: {check_type}"

        status = "ठीक" if ok else "विफल"
        print(f"  [{status}] {automation['name']}: {msg}")

        if not ok:
            failures.append(f"{automation['name']}: {msg}")

    if failures:
        alert_msg = (
            f"ऑटोमेशन हेल्थ चेक — {len(failures)} विफलता\n\n"
            + "\n".join(f"  {f}" for f in failures)
            + "\n\nलॉग चेक करें और जमा होने से पहले ठीक करें।"
        )
        send_alert(alert_msg)

if __name__ == "__main__":
    main()
```

इसे हर सुबह चलाएं। जब कोई ऑटोमेशन चुपचाप टूटती है (और टूटेगी), आपको 3 हफ्ते के बजाय 24 घंटे में पता चलेगा।

### रिव्यू क्यू बनाना

ह्यूमन-इन-द-लूप को एफिशिएंट बनाने की कुंजी अपनी रिव्यू को बैच करना है। जैसे ही आइटम आएं एक-एक करके रिव्यू न करें। उन्हें क्यू करें और बैच में रिव्यू करें।

```python
#!/usr/bin/env python3
"""
review_queue.py — AI-जनरेटेड आउटपुट के लिए सिंपल रिव्यू क्यू।
लगातार चेक करने के बजाय दिन में एक या दो बार रिव्यू करें।
"""

import os
import json
from datetime import datetime
from pathlib import Path

QUEUE_DIR = os.path.expanduser("~/income/review-queue")

def add_to_queue(item_type: str, content: dict):
    os.makedirs(QUEUE_DIR, exist_ok=True)
    timestamp = datetime.now().strftime("%Y%m%d-%H%M%S")
    filename = f"{timestamp}-{item_type}.json"
    filepath = os.path.join(QUEUE_DIR, filename)

    item = {
        "type": item_type,
        "created_at": datetime.utcnow().isoformat() + "Z",
        "status": "pending",
        "content": content
    }

    with open(filepath, "w") as f:
        json.dump(item, f, indent=2)

def review_queue():
    if not os.path.isdir(QUEUE_DIR):
        print("क्यू खाली है।")
        return

    pending = sorted(Path(QUEUE_DIR).glob("*.json"))

    if not pending:
        print("क्यू खाली है।")
        return

    print(f"\n{'='*60}")
    print(f"रिव्यू क्यू — {len(pending)} आइटम पेंडिंग")
    print(f"{'='*60}\n")

    for i, filepath in enumerate(pending):
        with open(filepath, "r") as f:
            item = json.load(f)

        print(f"[{i+1}] {item['type']} — {item['created_at']}")
        content = item.get("content", {})

        if item["type"] == "newsletter_draft":
            print(f"    आर्टिकल: {content.get('article_count', '?')}")
            print(f"    ड्राफ्ट: {content.get('draft_path', 'अज्ञात')}")
        elif item["type"] == "customer_response":
            print(f"    को: {content.get('customer', 'अज्ञात')}")
            print(f"    ड्राफ्ट: {content.get('draft_response', '')[:100]}...")
        elif item["type"] == "social_post":
            print(f"    टेक्स्ट: {content.get('text', '')[:100]}...")

        print(f"    एक्शन: [अ]प्रूव  [ए]डिट  [रि]जेक्ट  [स्कि]प")
        print()

if __name__ == "__main__":
    review_queue()
```

**रिव्यू आदत:** सुबह 8 बजे और शाम 4 बजे अपनी रिव्यू क्यू चेक करें। दो सेशन, हर एक 10-15 मिनट। बाकी सब रिव्यू के बीच ऑटोनॉमसली चलता है।

> **सीधी बात:** सोचिए क्या होता है जब आप ह्यूमन रिव्यू स्किप करते हैं: आप अपना न्यूजलेटर पूरी तरह ऑटोमेट करते हैं, LLM उन पेजों के हैलुसिनेटेड लिंक डालना शुरू कर देता है जो एक्सिस्ट नहीं करते, और सब्सक्राइबर आपसे पहले नोटिस करते हैं। आप अपनी लिस्ट का एक हिस्सा खो देते हैं और ट्रस्ट रीबिल्ड करने में महीने लगते हैं। इसके विपरीत, जो डेवलपर उसी प्रोसेस का 80% ऑटोमेट करता है — LLM क्यूरेट और ड्राफ्ट करता है, वे 10 मिनट रिव्यू करते हैं — वे उन हैलुसिनेशन को शिप होने से पहले पकड़ लेते हैं। अंतर ऑटोमेशन नहीं है। रिव्यू स्टेप है।

### आपकी बारी

1. पाठ 2 और 3 में बनाई ऑटोमेशन के लिए `automation_healthcheck.py` स्क्रिप्ट सेटअप करें। हर सुबह चलने के लिए शेड्यूल करें।
2. अपनी सबसे हाई-रिस्क ऑटोमेशन आउटपुट (कुछ भी कस्टमर-फेसिंग) के लिए रिव्यू क्यू इम्प्लीमेंट करें।
3. एक हफ्ते तक दिन में दो बार रिव्यू क्यू चेक करने के लिए कमिट करें। लॉग करें कितने आइटम अनचेंज्ड अप्रूव करते हैं, कितने एडिट करते हैं, और कितने रिजेक्ट करते हैं। यह डेटा बताता है कि आपकी ऑटोमेशन वास्तव में कितनी अच्छी है।

---

## पाठ 6: कॉस्ट ऑप्टिमाइजेशन और आपकी पहली पाइपलाइन

*"अगर $200 API खर्च से $200 रेवेन्यू जनरेट नहीं कर सकते, प्रोडक्ट ठीक करें — बजट नहीं।"*

### LLM-पावर्ड ऑटोमेशन की अर्थव्यवस्था

हर LLM कॉल की लागत होती है। लोकल मॉडल भी बिजली और GPU वियर कॉस्ट करते हैं। सवाल यह है कि क्या उस कॉल का आउटपुट कॉल की लागत से अधिक मूल्य जनरेट करता है।

{? if profile.gpu.exists ?}
{= profile.gpu.model | fallback("आपके GPU") =} पर लोकल मॉडल चलाने की लागत टिपिकल पाइपलाइन वर्कलोड के लिए बिजली में लगभग {= regional.currency_symbol | fallback("$") =}{= computed.monthly_electricity_estimate | fallback("कुछ डॉलर") =} प्रति माह है। यही बेसलाइन है जो API ऑल्टरनेटिव से बीट करनी है।
{? endif ?}

**{= regional.currency_symbol | fallback("$") =}200/माह API बजट नियम:**

अगर आप अपनी ऑटोमेशन के लिए API कॉल पर {= regional.currency_symbol | fallback("$") =}200/माह खर्च कर रहे हैं, उन ऑटोमेशन को कम से कम {= regional.currency_symbol | fallback("$") =}200/माह मूल्य जनरेट करना चाहिए — या तो डायरेक्ट रेवेन्यू या बचा हुआ समय जो आप कहीं और रेवेन्यू में कन्वर्ट करते हैं।

अगर नहीं कर रही हैं: समस्या API बजट नहीं है। पाइपलाइन डिज़ाइन या जिस प्रोडक्ट को सपोर्ट करती है वह है।

### कॉस्ट-पर-आउटपुट ट्रैकिंग

यह हर पाइपलाइन में जोड़ें:

```python
"""
cost_tracker.py — हर LLM कॉल की लागत और जनरेट होने वाले मूल्य को ट्रैक करें।
रियल कॉस्ट डेटा पाने के लिए अपनी पाइपलाइन में इम्पोर्ट करें।
"""

import os
import json
from datetime import datetime
from pathlib import Path

COST_LOG = os.path.expanduser("~/income/logs/llm_costs.jsonl")

PRICING = {
    "llama3.1:8b": {"input": 0.003, "output": 0.003},
    "llama3.1:70b": {"input": 0.01, "output": 0.01},
    "claude-sonnet-4-20250514": {"input": 3.00, "output": 15.00},
    "claude-3-5-haiku-20241022": {"input": 0.80, "output": 4.00},
    "gpt-4o-mini": {"input": 0.15, "output": 0.60},
    "gpt-4o": {"input": 2.50, "output": 10.00},
}

def log_cost(
    pipeline: str,
    model: str,
    input_tokens: int,
    output_tokens: int,
    revenue_generated: float = 0.0,
    item_id: str = ""
):
    prices = PRICING.get(model, {"input": 1.0, "output": 5.0})

    cost = (
        (input_tokens / 1_000_000 * prices["input"]) +
        (output_tokens / 1_000_000 * prices["output"])
    )

    entry = {
        "timestamp": datetime.utcnow().isoformat() + "Z",
        "pipeline": pipeline,
        "model": model,
        "input_tokens": input_tokens,
        "output_tokens": output_tokens,
        "cost_usd": round(cost, 6),
        "revenue_usd": revenue_generated,
        "item_id": item_id,
    }

    os.makedirs(os.path.dirname(COST_LOG), exist_ok=True)
    with open(COST_LOG, "a") as f:
        f.write(json.dumps(entry) + "\n")

    return cost

def monthly_report() -> dict:
    current_month = datetime.now().strftime("%Y-%m")
    pipelines = {}

    try:
        with open(COST_LOG, "r") as f:
            for line in f:
                entry = json.loads(line)
                if not entry["timestamp"].startswith(current_month):
                    continue

                pipeline = entry["pipeline"]
                if pipeline not in pipelines:
                    pipelines[pipeline] = {
                        "total_cost": 0,
                        "total_revenue": 0,
                        "call_count": 0,
                        "total_tokens": 0
                    }

                pipelines[pipeline]["total_cost"] += entry["cost_usd"]
                pipelines[pipeline]["total_revenue"] += entry.get("revenue_usd", 0)
                pipelines[pipeline]["call_count"] += 1
                pipelines[pipeline]["total_tokens"] += entry["input_tokens"] + entry["output_tokens"]
    except FileNotFoundError:
        pass

    print(f"\nLLM कॉस्ट रिपोर्ट — {current_month}")
    print("=" * 60)

    grand_cost = 0
    grand_revenue = 0

    for name, data in sorted(pipelines.items()):
        roi = data["total_revenue"] / data["total_cost"] if data["total_cost"] > 0 else 0
        print(f"\n  {name}")
        print(f"    कॉल:      {data['call_count']}")
        print(f"    टोकन:    {data['total_tokens']:,}")
        print(f"    लागत:     ${data['total_cost']:.4f}")
        print(f"    रेवेन्यू:  ${data['total_revenue']:.2f}")
        print(f"    ROI:      {roi:.1f}x")

        grand_cost += data["total_cost"]
        grand_revenue += data["total_revenue"]

    print(f"\n{'='*60}")
    print(f"  कुल लागत:     ${grand_cost:.4f}")
    print(f"  कुल रेवेन्यू:  ${grand_revenue:.2f}")
    if grand_cost > 0:
        print(f"  ओवरऑल ROI:   {grand_revenue/grand_cost:.1f}x")

    return pipelines

if __name__ == "__main__":
    monthly_report()
```

### API एफिशिएंसी के लिए बैचिंग

अगर API मॉडल उपयोग कर रहे हैं, बैचिंग असली पैसा बचाती है:

```python
"""
batch_api.py — एफिशिएंसी के लिए API कॉल बैच करें।
100 अलग-अलग API कॉल बनाने के बजाय, बैच करें।
"""

import os
import json
import time
import requests
from typing import Any

ANTHROPIC_KEY = os.environ.get("ANTHROPIC_API_KEY", "")

def batch_classify(
    items: list[dict],
    system_prompt: str,
    model: str = "claude-3-5-haiku-20241022",
    batch_size: int = 10,
    delay_between_batches: float = 1.0
) -> list[dict]:
    """
    सिंगल API कॉल में बैच करके कई आइटम एफिशिएंटली क्लासिफाई करें।

    100 API कॉल (100 आइटम × 1 कॉल प्रत्येक) के बजाय:
      - 100 कॉल × ~500 इनपुट टोकन = 50,000 इनपुट टोकन
      - 100 कॉल × ~200 आउटपुट टोकन = 20,000 आउटपुट टोकन
      - Haiku से लागत: ~$0.12

    बैचिंग से (10 आइटम प्रति कॉल, 10 API कॉल):
      - 10 कॉल × ~2,500 इनपुट टोकन = 25,000 इनपुट टोकन
      - 10 कॉल × ~1,000 आउटपुट टोकन = 10,000 आउटपुट टोकन
      - Haiku से लागत: ~$0.06

    सिर्फ बैचिंग से 50% बचत।
    """
    results = []

    for i in range(0, len(items), batch_size):
        batch = items[i:i + batch_size]

        items_text = "\n".join(
            f"[ITEM {j+1}] {json.dumps(item)}"
            for j, item in enumerate(batch)
        )

        prompt = f"""Process each item below. For each item, provide a JSON object with your classification.

{items_text}

Respond with a JSON array containing one object per item, in the same order.
Each object should have: {{"item_index": <number>, "category": "<string>", "score": <1-10>}}"""

        try:
            resp = requests.post(
                "https://api.anthropic.com/v1/messages",
                headers={
                    "x-api-key": ANTHROPIC_KEY,
                    "anthropic-version": "2023-06-01",
                    "content-type": "application/json"
                },
                json={
                    "model": model,
                    "max_tokens": 2000,
                    "system": system_prompt,
                    "messages": [{"role": "user", "content": prompt}]
                },
                timeout=60
            )
            resp.raise_for_status()

            response_text = resp.json()["content"][0]["text"]
            cleaned = response_text.strip()
            if cleaned.startswith("```"):
                cleaned = cleaned.split("\n", 1)[1].rsplit("```", 1)[0]

            batch_results = json.loads(cleaned)
            results.extend(batch_results)

        except Exception as e:
            print(f"  बैच {i//batch_size + 1} विफल: {e}")
            for item in batch:
                results.append({"item_index": i, "category": "unknown", "score": 0, "error": str(e)})

        if delay_between_batches > 0:
            time.sleep(delay_between_batches)

    return results
```

### कैशिंग: एक ही जवाब के लिए दो बार पेमेंट न करें

```python
"""
llm_cache.py — डुप्लिकेट प्रोसेसिंग से बचने के लिए LLM रिस्पॉन्स कैश करें।
"""

import os
import json
import hashlib
import sqlite3
from datetime import datetime, timedelta

CACHE_DB = os.path.expanduser("~/income/data/llm_cache.db")

def get_cache_db() -> sqlite3.Connection:
    os.makedirs(os.path.dirname(CACHE_DB), exist_ok=True)
    conn = sqlite3.connect(CACHE_DB)
    conn.execute("""
        CREATE TABLE IF NOT EXISTS cache (
            key TEXT PRIMARY KEY,
            model TEXT NOT NULL,
            response TEXT NOT NULL,
            created_at TEXT NOT NULL,
            hit_count INTEGER DEFAULT 0
        )
    """)
    conn.commit()
    return conn

def cache_key(model: str, prompt: str) -> str:
    return hashlib.sha256(f"{model}:{prompt}".encode()).hexdigest()

def get_cached(model: str, prompt: str, max_age_hours: int = 168) -> str | None:
    conn = get_cache_db()
    key = cache_key(model, prompt)

    row = conn.execute(
        "SELECT response, created_at FROM cache WHERE key = ?", (key,)
    ).fetchone()

    if row is None:
        conn.close()
        return None

    response, created_at = row
    age = datetime.utcnow() - datetime.fromisoformat(created_at)

    if age > timedelta(hours=max_age_hours):
        conn.execute("DELETE FROM cache WHERE key = ?", (key,))
        conn.commit()
        conn.close()
        return None

    conn.execute("UPDATE cache SET hit_count = hit_count + 1 WHERE key = ?", (key,))
    conn.commit()
    conn.close()
    return response

def set_cached(model: str, prompt: str, response: str):
    conn = get_cache_db()
    key = cache_key(model, prompt)

    conn.execute("""
        INSERT OR REPLACE INTO cache (key, model, response, created_at, hit_count)
        VALUES (?, ?, ?, ?, 0)
    """, (key, model, response, datetime.utcnow().isoformat()))
    conn.commit()
    conn.close()

def cache_stats():
    conn = get_cache_db()
    total = conn.execute("SELECT COUNT(*) FROM cache").fetchone()[0]
    total_hits = conn.execute("SELECT SUM(hit_count) FROM cache").fetchone()[0] or 0
    conn.close()
    print(f"कैश एंट्री: {total}")
    print(f"कुल कैश हिट: {total_hits}")
    print(f"अनुमानित बचत: ~${total_hits * 0.002:.2f} (प्रति कॉल रफ एवरेज)")
```

**अपनी पाइपलाइन में उपयोग करें:**

```python
from llm_cache import get_cached, set_cached

def llm_with_cache(model: str, prompt: str) -> str:
    cached = get_cached(model, prompt)
    if cached is not None:
        return cached  # मुफ्त!

    response = call_llm(model, prompt)
    set_cached(model, prompt, response)
    return response
```

बार-बार एक ही टाइप का कंटेंट प्रोसेस करने वाली पाइपलाइन (क्लासिफिकेशन, एक्सट्रैक्शन) के लिए, कैशिंग 30-50% API कॉल एलिमिनेट कर सकती है। यह आपकी मंथली बिल का 30-50% ऑफ है।

### अपनी पहली कंप्लीट पाइपलाइन बनाना: स्टेप बाय स्टेप

"मेरे पास एक मैनुअल वर्कफ्लो है" से "यह मेरे सोते समय चलता है" तक की पूरी प्रक्रिया।

**स्टेप 1: अपनी वर्तमान मैनुअल प्रक्रिया मैप करें।**

एक स्पेसिफिक इनकम स्ट्रीम के लिए हर स्टेप लिख दें। न्यूजलेटर का उदाहरण:

```
1. ब्राउज़र टैब में 15 RSS फीड खोलें (10 मिनट)
2. हेडलाइन स्कैन करें, दिलचस्प खोलें (20 मिनट)
3. 8-10 आर्टिकल डिटेल में पढ़ें (40 मिनट)
4. टॉप 5 के लिए सारांश लिखें (30 मिनट)
5. इंट्रो पैराग्राफ लिखें (10 मिनट)
6. ईमेल टूल में फॉर्मेट करें (15 मिनट)
7. लिस्ट को भेजें (5 मिनट)

कुल: ~2 घंटे 10 मिनट
```

**स्टेप 2: तीन सबसे ज्यादा समय लेने वाले स्टेप पहचानें।**

उदाहरण से: आर्टिकल पढ़ना (40 मिनट), सारांश लिखना (30 मिनट), हेडलाइन स्कैन करना (20 मिनट)।

**स्टेप 3: सबसे आसान पहले ऑटोमेट करें।**

हेडलाइन स्कैनिंग ऑटोमेट करना सबसे आसान है — यह क्लासिफिकेशन है। LLM रेलेवेंस स्कोर करता है, आप सिर्फ टॉप-स्कोर्ड पढ़ते हैं।

**स्टेप 4: बचा हुआ समय और क्वालिटी मापें।**

हेडलाइन स्कैनिंग ऑटोमेट करने के बाद:
- बचा समय: 20 मिनट
- क्वालिटी: आपकी मैनुअल पसंद से 90% सहमति
- नेट: 20 मिनट बचे, नगण्य क्वालिटी लॉस

**स्टेप 5: अगला स्टेप ऑटोमेट करें।**

अब सारांश लिखना ऑटोमेट करें। LLM ड्राफ्ट बनाता है, आप एडिट करते हैं।

**स्टेप 6: जब तक डिमिनिशिंग रिटर्न न आएं तब तक जारी रखें।**

```
ऑटोमेशन से पहले: 2 घंटे 10 मिनट प्रति न्यूजलेटर
लेवल 2 के बाद (शेड्यूल्ड फेचिंग): 1 घंटा 45 मिनट
लेवल 3 के बाद (LLM स्कोरिंग + सारांश): 25 मिनट
लेवल 3+ के बाद (LLM इंट्रो ड्राफ्ट): सिर्फ 10 मिनट रिव्यू

प्रति सप्ताह बचा समय: ~2 घंटे
प्रति माह बचा समय: ~8 घंटे
$100/घंटा इफेक्टिव रेट पर: $800/माह मुक्त समय
API लागत: $0 (लोकल LLM) से $5/माह (API)
```

**स्टेप 7: कंप्लीट पाइपलाइन, वायर्ड टुगेदर।**

वीकली न्यूजलेटर पाइपलाइन के लिए सब कुछ जोड़ने वाला GitHub Action:

```yaml
# .github/workflows/newsletter-pipeline.yml
name: Weekly Newsletter Pipeline

on:
  schedule:
    # हर रविवार सुबह 5 बजे UTC
    - cron: '0 5 * * 0'
  workflow_dispatch:

jobs:
  generate-newsletter:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Set up Python
        uses: actions/setup-python@v5
        with:
          python-version: '3.12'

      - name: Install dependencies
        run: pip install requests

      - name: Run newsletter pipeline
        env:
          ANTHROPIC_API_KEY: ${{ secrets.ANTHROPIC_API_KEY }}
          NEWSLETTER_NICHE: "Rust developers, local AI, developer tooling"
        run: python scripts/newsletter_pipeline.py

      - name: Upload draft as artifact
        uses: actions/upload-artifact@v4
        with:
          name: newsletter-draft-${{ github.run_number }}
          path: drafts/

      - name: Notify via Slack
        if: success()
        run: |
          curl -X POST "${{ secrets.SLACK_WEBHOOK }}" \
            -H 'Content-Type: application/json' \
            -d '{"text":"न्यूजलेटर ड्राफ्ट रिव्यू के लिए तैयार। GitHub Actions आर्टिफैक्ट चेक करें।"}'
```

यह हर रविवार सुबह 5 बजे चलता है। जब तक आप उठते हैं, ड्राफ्ट इंतजार कर रहा होता है। आप कॉफी पीते हुए 10 मिनट रिव्यू करते हैं, सेंड दबाते हैं, और आपका न्यूजलेटर हफ्ते के लिए पब्लिश हो जाता है।

### आपकी बारी: अपनी पाइपलाइन बनाएं

यह मॉड्यूल का डिलिवरेबल है। इस पाठ के अंत तक, आपके पास एक कंप्लीट पाइपलाइन डिप्लॉय होकर चल रही होनी चाहिए।

**आपकी पाइपलाइन की आवश्यकताएं:**
1. यह बिना आपकी भागीदारी के शेड्यूल पर चलती है
2. इसमें कम से कम एक LLM प्रोसेसिंग स्टेप है
3. इसमें क्वालिटी कंट्रोल के लिए ह्यूमन रिव्यू स्टेप है
4. इसमें हेल्थ चेक है ताकि टूटने पर पता चले
5. यह रियल इनकम स्ट्रीम (या बन रही स्ट्रीम) से जुड़ी है

**चेकलिस्ट:**

- [ ] ऑटोमेट करने के लिए इनकम स्ट्रीम चुनी
- [ ] मैनुअल प्रोसेस मैप किया (सभी स्टेप, टाइम एस्टिमेट के साथ)
- [ ] 3 सबसे ज्यादा समय लेने वाले स्टेप पहचाने
- [ ] कम से कम पहला स्टेप ऑटोमेट किया (क्लासिफिकेशन/स्कोरिंग/फिल्टरिंग)
- [ ] दूसरे स्टेप के लिए LLM प्रोसेसिंग जोड़ी (सारांश/जनरेशन/एक्सट्रैक्शन)
- [ ] ह्यूमन ओवरसाइट के लिए रिव्यू क्यू बनाई
- [ ] ऑटोमेशन के लिए हेल्थ चेक सेटअप किया
- [ ] शेड्यूल पर डिप्लॉय किया (cron, GitHub Actions, या systemd टाइमर)
- [ ] एक पूरे साइकल के लिए कॉस्ट और टाइम सेविंग ट्रैक की
- [ ] पाइपलाइन डॉक्यूमेंट की (क्या करती है, कैसे ठीक करें, क्या मॉनिटर करें)

अगर आपने इस चेकलिस्ट के सभी दस आइटम पूरे कर लिए, तो आपके पास लेवल 3 ऑटोमेशन चल रही है। आपने अपने हफ्ते के घंटे मुक्त कर लिए जो आप अधिक स्ट्रीम बनाने या मौजूदा सुधारने में रीइन्वेस्ट कर सकते हैं।

---

## मॉड्यूल T: पूर्ण

{@ temporal automation_progress @}

### दो हफ्तों में आपने क्या बनाया

1. **ऑटोमेशन पिरामिड की समझ** — आप जानते हैं कि आप कहां हैं और आपकी हर इनकम स्ट्रीम किस दिशा में जानी चाहिए।
2. **शेड्यूल्ड ऑटोमेशन** cron या क्लाउड शेड्यूलर पर चल रही हैं — वह अनग्लैमरस फाउंडेशन जो बाकी सब संभव बनाती है।
3. **LLM-पावर्ड पाइपलाइन** जो उन जज्मेंट कॉल्स को हैंडल करती हैं जो आप मैन्युअली लिया करते थे — क्लासिफाई करना, सारांशित करना, जनरेट करना, मॉनिटर करना।
4. **एजेंट-बेस्ड पैटर्न** जो आप कस्टमर इंटरैक्शन, फीडबैक प्रोसेसिंग, और MCP-पावर्ड प्रोडक्ट के लिए डिप्लॉय कर सकते हैं।
5. **ह्यूमन-इन-द-लूप फ्रेमवर्क** जो आपकी रेपुटेशन प्रोटेक्ट करता है जबकि आपका 80%+ समय बचाता है।
6. **कॉस्ट ट्रैकिंग और ऑप्टिमाइजेशन** ताकि आपकी ऑटोमेशन प्रॉफिट जनरेट करे, सिर्फ एक्टिविटी नहीं।
7. **एक कंप्लीट, डिप्लॉय पाइपलाइन** बिना आपकी सक्रिय भागीदारी के मूल्य जनरेट कर रही है।

### कंपाउंड इफेक्ट

अगले 3 महीनों में क्या होता है अगर आप इस मॉड्यूल में बनाया गया मेंटेन और एक्सटेंड करते हैं:

```
महीना 1: एक पाइपलाइन, 5-8 घंटे/सप्ताह बचा रही
महीना 2: दो पाइपलाइन, 10-15 घंटे/सप्ताह बचा रही
महीना 3: तीन पाइपलाइन, 15-20 घंटे/सप्ताह बचा रही

$100/घंटा इफेक्टिव रेट पर, यह $1,500-2,000/माह
मुक्त समय — वह समय जो आप नई स्ट्रीम में इन्वेस्ट करते हैं।

महीना 1 का मुक्त समय महीना 2 की पाइपलाइन बनाता है।
महीना 2 का मुक्त समय महीना 3 की पाइपलाइन बनाता है।
ऑटोमेशन कंपाउंड करता है।
```

यही है कि एक डेवलपर पांच लोगों की टीम जैसे कैसे ऑपरेट करता है। ज्यादा मेहनत करके नहीं। ऐसे सिस्टम बनाकर जो तब काम करते हैं जब आप नहीं करते।

---

### 4DA इंटीग्रेशन

{? if dna.identity_summary ?}
आपकी डेवलपर प्रोफाइल — {= dna.identity_summary | fallback("आपका डेवलपमेंट फोकस") =} — के आधार पर, नीचे 4DA टूल्स सीधे उन ऑटोमेशन पैटर्न से मैप होते हैं जो आपने अभी सीखे। सिग्नल क्लासिफिकेशन टूल्स आपके स्पेस के डेवलपर्स के लिए विशेष रूप से रेलेवेंट हैं।
{? endif ?}

4DA खुद एक लेवल 3 ऑटोमेशन है। यह दर्जनों सोर्स से कंटेंट इंजेस्ट करती है, हर आइटम को PASIFA एल्गोरिदम से स्कोर करती है, और सिर्फ वही सरफेस करती है जो आपके काम से रेलेवेंट है — बिना आपकी उंगली उठाए। आप मैन्युअली Hacker News, Reddit, और 50 RSS फीड चेक नहीं करते। 4DA करती है और आपको दिखाती है क्या मायने रखता है।

अपनी इनकम पाइपलाइन उसी तरह बनाएं।

4DA की अटेंशन रिपोर्ट (`/attention_report` MCP टूल्स में) आपको दिखाती है कि आपका समय वास्तव में कहां जाता है बनाम कहां जाना चाहिए। क्या ऑटोमेट करना है यह तय करने से पहले इसे चलाएं। "समय खर्च" और "समय खर्च होना चाहिए" के बीच का गैप आपका ऑटोमेशन रोडमैप है।

सिग्नल क्लासिफिकेशन टूल्स (`/get_actionable_signals`) सीधे आपकी मार्केट मॉनिटरिंग पाइपलाइन में फीड कर सकते हैं — 4DA की इंटेलिजेंस लेयर को आपकी कस्टम पाइपलाइन से पहले इनिशियल स्कोरिंग करने दें जो निश-स्पेसिफिक एनालिसिस करती है।

अगर आप सोर्स मॉनिटर करने वाली पाइपलाइन बना रहे हैं, तो 4DA जो पहले से करती है उसे दोबारा आविष्कार न करें। अपनी ऑटोमेशन स्टैक में बिल्डिंग ब्लॉक के रूप में इसके MCP सर्वर का उपयोग करें।

---

### आगे क्या: मॉड्यूल S — स्ट्रीम स्टैकिंग

मॉड्यूल T ने आपको हर इनकम स्ट्रीम को एफिशिएंटली चलाने के टूल दिए। मॉड्यूल S (स्ट्रीम स्टैकिंग) अगले सवाल का जवाब देता है: **कितनी स्ट्रीम चलानी चाहिए, और वे एक साथ कैसे फिट होती हैं?**

मॉड्यूल S में क्या कवर होता है:

- **इनकम स्ट्रीम के लिए पोर्टफोलियो थ्योरी** — 3 स्ट्रीम 1 स्ट्रीम को क्यों हराती हैं, और 10 स्ट्रीम शून्य को क्यों हराती हैं
- **स्ट्रीम कोरिलेशन** — कौन सी स्ट्रीम एक दूसरे को रीइनफोर्स करती हैं और कौन सी आपके समय के लिए कंपीट करती हैं
- **इनकम फ्लोर** — एक्सपेरिमेंट करने से पहले रिकरिंग रेवेन्यू का बेस बनाना जो आपकी कॉस्ट कवर करे
- **रीबैलेंसिंग** — विनर पर कब डबल डाउन करना है और अंडरपरफॉर्मर को कब खत्म करना है
- **$10K/माह आर्किटेक्चर** — 15-20 घंटे प्रति सप्ताह में पांच अंकों तक पहुंचने वाले स्पेसिफिक स्ट्रीम कॉम्बिनेशन

आपके पास इंफ्रास्ट्रक्चर (मॉड्यूल S), मोट्स (मॉड्यूल T), इंजन (मॉड्यूल R), लॉन्च प्लेबुक (मॉड्यूल E), ट्रेंड रडार (मॉड्यूल E), और अब ऑटोमेशन (मॉड्यूल T) है। मॉड्यूल S इन सबको एक सस्टेनेबल, ग्रोइंग इनकम पोर्टफोलियो में टाई करता है।

---

**पाइपलाइन चलती है। ड्राफ्ट तैयार है। आप 10 मिनट रिव्यू करते हैं।**

**यही है टैक्टिकल ऑटोमेशन। ऐसे स्केल करते हैं।**
