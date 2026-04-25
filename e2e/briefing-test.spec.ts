import { test, expect } from '@playwright/test';

test('trigger morning briefing and capture output', async ({ page }) => {
  // Collect ALL console messages
  const consoleLogs: string[] = [];
  page.on('console', (msg) => {
    const text = `[${msg.type()}] ${msg.text()}`;
    consoleLogs.push(text);
  });

  // Step 1: Navigate to the app
  console.log('--- Step 1: Navigating to http://localhost:4444 ---');
  await page.goto('http://localhost:4444', { waitUntil: 'networkidle', timeout: 30000 });
  await page.waitForTimeout(2000); // Let the app fully render

  // Step 2: Take initial screenshot
  console.log('--- Step 2: Taking initial screenshot ---');
  await page.screenshot({ path: '/d/4DA/e2e-results/briefing-01-before.png', fullPage: true });

  // Step 3: Execute __testBriefing() in the browser console
  console.log('--- Step 3: Triggering __testBriefing() ---');
  
  // First check if the function exists
  const fnExists = await page.evaluate(() => {
    return typeof (window as any).__testBriefing === 'function';
  });
  console.log(`__testBriefing exists: ${fnExists}`);

  let triggerResult: any = null;
  let triggerError: string | null = null;

  if (fnExists) {
    try {
      triggerResult = await page.evaluate(async () => {
        try {
          const result = await (window as any).__testBriefing();
          return { success: true, result: JSON.stringify(result, null, 2) };
        } catch (err: any) {
          return { success: false, error: err.message || String(err) };
        }
      });
      console.log('Trigger result:', JSON.stringify(triggerResult, null, 2));
    } catch (err: any) {
      triggerError = err.message;
      console.log('Trigger threw:', triggerError);
    }
  } else {
    console.log('__testBriefing not found, trying window.__testBriefing...');
    // Try alternative approaches
    const altCheck = await page.evaluate(() => {
      const keys = Object.keys(window).filter(k => k.toLowerCase().includes('brief') || k.toLowerCase().includes('test'));
      return keys;
    });
    console.log('Window keys matching brief/test:', altCheck);
  }

  // Step 4: Wait for LLM synthesis to complete (15-20 seconds)
  console.log('--- Step 4: Waiting 20 seconds for LLM synthesis ---');
  await page.waitForTimeout(20000);

  // Step 5: Take post-briefing screenshot
  console.log('--- Step 5: Taking post-briefing screenshot ---');
  await page.screenshot({ path: '/d/4DA/e2e-results/briefing-02-after.png', fullPage: true });

  // Step 6: Check console logs
  console.log('--- Step 6: Console logs collected ---');
  console.log(`Total console messages: ${consoleLogs.length}`);
  
  // Filter for briefing-related logs
  const briefingLogs = consoleLogs.filter(l => 
    l.toLowerCase().includes('briefing') || 
    l.toLowerCase().includes('synthesis') ||
    l.toLowerCase().includes('morning') ||
    l.toLowerCase().includes('trigger') ||
    l.toLowerCase().includes('enrichment') ||
    l.toLowerCase().includes('pipeline') ||
    l.toLowerCase().includes('error') ||
    l.toLowerCase().includes('warn')
  );
  
  console.log(`\nBriefing-related logs (${briefingLogs.length}):`);
  briefingLogs.forEach(l => console.log('  ', l));
  
  console.log(`\nAll console logs:`);
  consoleLogs.forEach(l => console.log('  ', l));

  // Step 7: Get visible page text
  console.log('--- Step 7: Getting visible page text ---');
  const bodyText = await page.evaluate(() => {
    return document.body?.innerText?.substring(0, 5000) || 'NO BODY TEXT';
  });
  console.log('Page text (first 5000 chars):\n', bodyText);

  // Check for any new windows/tabs
  const pages = page.context().pages();
  console.log(`\nOpen pages: ${pages.length}`);
  for (let i = 0; i < pages.length; i++) {
    console.log(`  Page ${i}: ${pages[i].url()}`);
  }

  // Final: write a summary
  console.log('\n=== SUMMARY ===');
  console.log(`Function exists: ${fnExists}`);
  console.log(`Trigger result: ${JSON.stringify(triggerResult)}`);
  console.log(`Trigger error: ${triggerError}`);
  console.log(`Console messages: ${consoleLogs.length}`);
  console.log(`Briefing-related: ${briefingLogs.length}`);
});
