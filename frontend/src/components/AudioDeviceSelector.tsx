'use client';

import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Mic, Speaker, Headphones, Check } from 'lucide-react';

interface AudioDevice {
  name: string;
  device_type: 'Input' | 'Output';
}

interface AudioDeviceSelectorProps {
  onDeviceChange?: (micDevice: string | null, systemAudioEnabled: boolean) => void;
}

export const AudioDeviceSelector: React.FC<AudioDeviceSelectorProps> = ({
  onDeviceChange
}) => {
  const [devices, setDevices] = useState<AudioDevice[]>([]);
  const [selectedMic, setSelectedMic] = useState<string | null>(null);
  const [systemAudioEnabled, setSystemAudioEnabled] = useState(true);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const loadAudioDevices = async () => {
    try {
      setIsLoading(true);
      setError(null);
      const audioDevices = await invoke<AudioDevice[]>('get_audio_devices');
      setDevices(audioDevices);
      console.log('Audio devices loaded:', audioDevices);
    } catch (err) {
      console.error('Failed to load audio devices:', err);
      setError('Failed to load audio devices. Please check permissions.');
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => {
    loadAudioDevices();
    
    // Load saved settings
    const savedMicDevice = localStorage.getItem('selectedMicDevice');
    const savedSystemAudio = localStorage.getItem('systemAudioEnabled');
    
    if (savedMicDevice) {
      setSelectedMic(savedMicDevice);
    }
    if (savedSystemAudio !== null) {
      setSystemAudioEnabled(savedSystemAudio === 'true');
    }
  }, []);

  useEffect(() => {
    if (onDeviceChange) {
      onDeviceChange(selectedMic, systemAudioEnabled);
    }
  }, [selectedMic, systemAudioEnabled, onDeviceChange]);

  const micDevices = devices.filter(d => d.device_type === 'Input');
  const outputDevices = devices.filter(d => d.device_type === 'Output');

  const getDeviceIcon = (deviceName: string) => {
    const name = deviceName.toLowerCase();
    if (name.includes('airpods') || name.includes('bluetooth') || name.includes('wireless')) {
      return <Headphones className="w-4 h-4" />;
    }
    return <Mic className="w-4 h-4" />;
  };

  if (isLoading) {
    return (
      <div className="p-4 border rounded-lg">
        <h3 className="text-lg font-semibold mb-4 flex items-center gap-2">
          <Mic className="w-5 h-5" />
          Audio Devices
        </h3>
        <div className="flex items-center gap-2">
          <div className="animate-spin rounded-full h-4 w-4 border-b-2 border-gray-900"></div>
          <span className="text-sm text-gray-600">Loading audio devices...</span>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="p-4 border rounded-lg">
        <h3 className="text-lg font-semibold mb-4 flex items-center gap-2">
          <Mic className="w-5 h-5" />
          Audio Devices
        </h3>
        <div className="text-red-600 text-sm mb-2">{error}</div>
        <button 
          onClick={loadAudioDevices}
          className="text-blue-600 hover:text-blue-800 text-sm underline"
        >
          Retry
        </button>
      </div>
    );
  }

  return (
    <div className="p-4 border rounded-lg space-y-6">
      <h3 className="text-lg font-semibold flex items-center gap-2">
        <Mic className="w-5 h-5" />
        Audio Devices
      </h3>

      {/* Microphone Selection */}
      <div className="space-y-3">
        <h4 className="font-medium text-gray-800 flex items-center gap-2">
          <Mic className="w-4 h-4" />
          Microphone
        </h4>
        <div className="space-y-2 max-h-32 overflow-y-auto">
          {micDevices.length === 0 ? (
            <div className="text-sm text-gray-500 italic">No microphones found</div>
          ) : (
            <>
              <label className="flex items-center gap-2 p-2 hover:bg-gray-50 rounded cursor-pointer">
                <input
                  type="radio"
                  name="microphone"
                  checked={selectedMic === null}
                  onChange={() => setSelectedMic(null)}
                  className="sr-only"
                />
                <div className={`w-4 h-4 rounded-full border-2 flex items-center justify-center ${
                  selectedMic === null ? 'border-blue-500 bg-blue-500' : 'border-gray-300'
                }`}>
                  {selectedMic === null && <Check className="w-2 h-2 text-white" />}
                </div>
                <Mic className="w-4 h-4 text-gray-500" />
                <span className="text-sm">Use Default Microphone</span>
              </label>
              {micDevices.map((device) => (
                <label key={device.name} className="flex items-center gap-2 p-2 hover:bg-gray-50 rounded cursor-pointer">
                  <input
                    type="radio"
                    name="microphone"
                    checked={selectedMic === device.name}
                    onChange={() => setSelectedMic(device.name)}
                    className="sr-only"
                  />
                  <div className={`w-4 h-4 rounded-full border-2 flex items-center justify-center ${
                    selectedMic === device.name ? 'border-blue-500 bg-blue-500' : 'border-gray-300'
                  }`}>
                    {selectedMic === device.name && <Check className="w-2 h-2 text-white" />}
                  </div>
                  {getDeviceIcon(device.name)}
                  <span className="text-sm truncate" title={device.name}>
                    {device.name}
                  </span>
                </label>
              ))}
            </>
          )}
        </div>
      </div>

      {/* System Audio Toggle */}
      <div className="space-y-3">
        <h4 className="font-medium text-gray-800 flex items-center gap-2">
          <Speaker className="w-4 h-4" />
          System Audio
        </h4>
        <label className="flex items-center gap-2 p-2 hover:bg-gray-50 rounded cursor-pointer">
          <input
            type="checkbox"
            checked={systemAudioEnabled}
            onChange={(e) => setSystemAudioEnabled(e.target.checked)}
            className="sr-only"
          />
          <div className={`w-4 h-4 rounded border-2 flex items-center justify-center ${
            systemAudioEnabled ? 'border-blue-500 bg-blue-500' : 'border-gray-300'
          }`}>
            {systemAudioEnabled && <Check className="w-2 h-2 text-white" />}
          </div>
          <Speaker className="w-4 h-4 text-gray-500" />
          <span className="text-sm">Capture system audio (speakers, music, videos)</span>
        </label>
      </div>

      {/* Device Info */}
      <div className="text-xs text-gray-500 bg-gray-50 p-3 rounded">
        <p><strong>Found:</strong> {micDevices.length} microphone(s), {outputDevices.length} output device(s)</p>
        <p><strong>Note:</strong> AirPods and Bluetooth devices are now supported!</p>
      </div>
    </div>
  );
};