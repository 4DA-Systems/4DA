import { useCallback } from 'react';
import { useAppStore } from '../store';
import { translateError } from '../utils/error-messages';

/**
 * Monitoring toggle/interval action handlers for the SettingsModal.
 * These wrap store actions with status message feedback.
 */
export function useSettingsModalActions() {
  const setSettingsStatus = useAppStore(s => s.setSettingsStatus);
  const toggleMonitoring = useAppStore(s => s.toggleMonitoring);
  const updateMonitoringInterval = useAppStore(s => s.updateMonitoringInterval);
  const testNotification = useAppStore(s => s.testNotification);

  const handleToggleMonitoring = useCallback(async () => {
    try {
      const msg = await toggleMonitoring();
      setSettingsStatus(msg);
      setTimeout(() => setSettingsStatus(''), 2000);
    } catch (error) {
      setSettingsStatus(`Error: ${translateError(error)}`);
    }
  }, [toggleMonitoring, setSettingsStatus]);

  const handleUpdateMonitoringInterval = useCallback(async () => {
    try {
      const msg = await updateMonitoringInterval();
      setSettingsStatus(msg);
      setTimeout(() => setSettingsStatus(''), 2000);
    } catch (error) {
      setSettingsStatus(`Error: ${translateError(error)}`);
    }
  }, [updateMonitoringInterval, setSettingsStatus]);

  const handleTestNotification = useCallback(async () => {
    try {
      const msg = await testNotification();
      setSettingsStatus(msg);
      setTimeout(() => setSettingsStatus(''), 2000);
    } catch (error) {
      setSettingsStatus(`Notification error: ${translateError(error)}`);
    }
  }, [testNotification, setSettingsStatus]);

  return {
    handleToggleMonitoring,
    handleUpdateMonitoringInterval,
    handleTestNotification,
  };
}
