import { useState, useEffect } from 'react';

export interface AudioDeviceSettings {
  selectedMicDevice: string | null;
  systemAudioEnabled: boolean;
}

export const useAudioDeviceSettings = () => {
  const [settings, setSettings] = useState<AudioDeviceSettings>({
    selectedMicDevice: null,
    systemAudioEnabled: true
  });

  // Load settings from localStorage on mount
  useEffect(() => {
    const savedMicDevice = localStorage.getItem('selectedMicDevice');
    const savedSystemAudio = localStorage.getItem('systemAudioEnabled');
    
    setSettings({
      selectedMicDevice: savedMicDevice || null,
      systemAudioEnabled: savedSystemAudio !== 'false' // Default to true unless explicitly false
    });
  }, []);

  const updateSettings = (newSettings: Partial<AudioDeviceSettings>) => {
    setSettings(prev => {
      const updated = { ...prev, ...newSettings };
      
      // Save to localStorage
      if (updated.selectedMicDevice !== undefined) {
        localStorage.setItem('selectedMicDevice', updated.selectedMicDevice || '');
      }
      localStorage.setItem('systemAudioEnabled', updated.systemAudioEnabled.toString());
      
      return updated;
    });
  };

  return {
    settings,
    updateSettings
  };
};