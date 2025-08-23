import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { 
  ModelInfo, 
  ModelStatus, 
  getModelIcon, 
  getStatusColor, 
  formatFileSize, 
  MODEL_CONFIGS 
} from '../types/whisper';
import { ModelDownloadProgress, ProgressRing, DownloadSummary } from './ModelDownloadProgress';

interface ModelManagerProps {
  selectedModel?: string;
  onModelSelect?: (modelName: string) => void;
  className?: string;
}

export function ModelManager({ selectedModel, onModelSelect, className = '' }: ModelManagerProps) {
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [downloadingModels, setDownloadingModels] = useState<Set<string>>(new Set());

  useEffect(() => {
    loadAvailableModels();
  }, []);

  const loadAvailableModels = async () => {
    try {
      setLoading(true);
      setError(null);
      
      // For now, we'll use mock data until the Tauri commands are implemented
      const mockModels: ModelInfo[] = [
        {
          name: 'large-v3',
          path: 'backend/models/ggml-large-v3.bin',
          sizeMb: 3000,
          accuracy: 'High',
          speed: 'Slow',
          status: 'Available', // This model exists
          description: MODEL_CONFIGS['large-v3'].description
        },
        {
          name: 'medium',
          path: 'backend/models/ggml-medium.bin',
          sizeMb: 1400,
          accuracy: 'Good',
          speed: 'Medium',
          status: 'Available', // This model exists
          description: MODEL_CONFIGS['medium'].description
        },
        {
          name: 'small',
          path: 'backend/models/ggml-small.bin',
          sizeMb: 465,
          accuracy: 'Decent',
          speed: 'Fast',
          status: 'Available', // This model exists
          description: MODEL_CONFIGS['small'].description
        }
      ];

      // TODO: Replace with actual Tauri command when backend is ready
      // const modelList = await invoke('get_available_models') as ModelInfo[];
      setModels(mockModels);
      
      // Auto-select best available model if none selected
      if (!selectedModel) {
        const availableModel = mockModels.find(m => m.status === 'Available');
        if (availableModel && onModelSelect) {
          onModelSelect(availableModel.name);
        }
      }
    } catch (err) {
      console.error('Failed to load models:', err);
      setError(err instanceof Error ? err.message : 'Failed to load models');
    } finally {
      setLoading(false);
    }
  };

  const downloadModel = async (modelName: string) => {
    try {
      setDownloadingModels(prev => new Set([...prev, modelName]));
      
      // Update model status to show download progress
      setModels(prev => prev.map(model => 
        model.name === modelName 
          ? { ...model, status: { Downloading: 0 } }
          : model
      ));

      // TODO: Replace with actual Tauri command when backend is ready
      // await invoke('download_model', { modelName });
      
      // Simulate download progress for demo
      for (let progress = 0; progress <= 100; progress += 10) {
        await new Promise(resolve => setTimeout(resolve, 200));
        setModels(prev => prev.map(model => 
          model.name === modelName 
            ? { ...model, status: { Downloading: progress } }
            : model
        ));
      }

      // Mark as available after download
      setModels(prev => prev.map(model => 
        model.name === modelName 
          ? { ...model, status: 'Available' }
          : model
      ));

      setDownloadingModels(prev => {
        const newSet = new Set(prev);
        newSet.delete(modelName);
        return newSet;
      });

      // Auto-select the downloaded model
      if (onModelSelect) {
        onModelSelect(modelName);
      }
    } catch (err) {
      console.error('Failed to download model:', err);
      setModels(prev => prev.map(model => 
        model.name === modelName 
          ? { ...model, status: { Error: 'Download failed' } }
          : model
      ));
      setDownloadingModels(prev => {
        const newSet = new Set(prev);
        newSet.delete(modelName);
        return newSet;
      });
    }
  };

  const selectModel = async (modelName: string) => {
    if (onModelSelect) {
      onModelSelect(modelName);
    }

    // TODO: Switch model in backend when ready
    // try {
    //   await invoke('switch_whisper_model', { modelName });
    // } catch (err) {
    //   console.error('Failed to switch model:', err);
    // }
  };

  const getStatusBadge = (status: ModelStatus, modelName: string) => {
    if (status === 'Available') {
      return (
        <div className="flex items-center space-x-1">
          <div className="w-2 h-2 bg-green-500 rounded-full"></div>
          <span className="text-xs text-green-700 font-medium">Ready</span>
        </div>
      );
    } else if (status === 'Missing') {
      return (
        <div className="flex items-center space-x-1">
          <div className="w-2 h-2 bg-gray-400 rounded-full"></div>
          <span className="text-xs text-gray-600">Not Downloaded</span>
        </div>
      );
    } else if (typeof status === 'object' && 'Downloading' in status) {
      return <ProgressRing progress={status.Downloading} size={24} strokeWidth={2} />;
    } else if (typeof status === 'object' && 'Error' in status) {
      return (
        <div className="flex items-center space-x-1">
          <div className="w-2 h-2 bg-red-500 rounded-full"></div>
          <span className="text-xs text-red-700">Error</span>
        </div>
      );
    }
    return null;
  };

  const availableModels = models.filter(m => m.status === 'Available');
  const totalSizeMb = availableModels.reduce((sum, m) => sum + m.sizeMb, 0);

  if (loading) {
    return (
      <div className={`animate-pulse space-y-4 ${className}`}>
        <div className="h-4 bg-gray-200 rounded w-3/4"></div>
        <div className="space-y-3">
          {[1, 2, 3].map(i => (
            <div key={i} className="h-20 bg-gray-200 rounded-lg"></div>
          ))}
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className={`bg-red-50 border border-red-200 rounded-lg p-4 ${className}`}>
        <div className="flex items-center space-x-2">
          <span className="text-red-600">❌</span>
          <div>
            <h4 className="font-medium text-red-800">Failed to load models</h4>
            <p className="text-sm text-red-700">{error}</p>
          </div>
        </div>
        <button
          onClick={loadAvailableModels}
          className="mt-3 text-sm text-red-600 hover:text-red-800 underline"
        >
          Try again
        </button>
      </div>
    );
  }

  return (
    <div className={`space-y-4 ${className}`}>
      <div className="bg-blue-50 border border-blue-200 rounded-lg p-4">
        <h3 className="font-semibold text-blue-900 mb-2 flex items-center">
          <span className="mr-2">🏠</span>
          Local Whisper Models
        </h3>
        <p className="text-sm text-blue-700 mb-3">
          High-quality speech recognition that runs entirely on your device
        </p>
        <DownloadSummary 
          totalModels={models.length}
          downloadedModels={availableModels.length}
          totalSizeMb={totalSizeMb}
        />
      </div>

      <div className="grid gap-4">
        {models.map((model) => {
          const isSelected = selectedModel === model.name;
          const isDownloading = typeof model.status === 'object' && 'Downloading' in model.status;
          const isAvailable = model.status === 'Available';
          
          return (
            <div key={model.name} className="space-y-2">
              <div 
                className={`p-4 border rounded-lg transition-all cursor-pointer ${
                  isSelected 
                    ? 'border-blue-500 bg-blue-50 shadow-sm' 
                    : 'border-gray-200 hover:border-gray-300 hover:shadow-sm'
                } ${!isAvailable && !isDownloading ? 'opacity-75' : ''}`}
                onClick={() => isAvailable && selectModel(model.name)}
              >
                <div className="flex justify-between items-start">
                  <div className="flex-1 pr-4">
                    <div className="flex items-center space-x-3 mb-2">
                      <span className="text-2xl">{getModelIcon(model.accuracy)}</span>
                      <div>
                        <h4 className="font-medium text-gray-900 flex items-center space-x-2">
                          <span>Whisper {model.name}</span>
                          {isSelected && (
                            <span className="bg-blue-600 text-white px-2 py-1 rounded-full text-xs">
                              Active
                            </span>
                          )}
                        </h4>
                        <p className="text-xs text-gray-600 mt-1">{model.description}</p>
                      </div>
                    </div>
                    
                    <div className="flex items-center space-x-4 text-sm text-gray-600">
                      <span className="flex items-center space-x-1">
                        <span>📦</span>
                        <span>{formatFileSize(model.sizeMb)}</span>
                      </span>
                      <span className="flex items-center space-x-1">
                        <span>🎯</span>
                        <span>{model.accuracy} accuracy</span>
                      </span>
                      <span className="flex items-center space-x-1">
                        <span>⚡</span>
                        <span>{model.speed} processing</span>
                      </span>
                    </div>
                  </div>

                  <div className="flex flex-col items-end space-y-2">
                    {getStatusBadge(model.status, model.name)}
                    
                    {model.status === 'Missing' && (
                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          downloadModel(model.name);
                        }}
                        className="bg-blue-600 text-white px-3 py-1 rounded text-xs hover:bg-blue-700 transition-colors"
                      >
                        Download
                      </button>
                    )}
                    
                    {typeof model.status === 'object' && 'Error' in model.status && (
                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          downloadModel(model.name);
                        }}
                        className="bg-red-600 text-white px-3 py-1 rounded text-xs hover:bg-red-700 transition-colors"
                      >
                        Retry
                      </button>
                    )}
                  </div>
                </div>
              </div>

              {isDownloading && (
                <ModelDownloadProgress
                  status={model.status}
                  modelName={model.name}
                  onCancel={() => {
                    // TODO: Implement cancel functionality
                    console.log('Cancel download:', model.name);
                  }}
                />
              )}
            </div>
          );
        })}
      </div>

      {selectedModel && availableModels.length > 0 && (
        <div className="bg-green-50 border border-green-200 rounded-lg p-3">
          <div className="flex items-center space-x-2">
            <span className="text-green-600">✓</span>
            <p className="text-sm text-green-800">
              Using <strong>{selectedModel}</strong> model for transcription
            </p>
          </div>
        </div>
      )}

      {availableModels.length === 0 && (
        <div className="bg-yellow-50 border border-yellow-200 rounded-lg p-4">
          <div className="flex items-center space-x-2">
            <span className="text-yellow-600">⚠️</span>
            <div>
              <h4 className="font-medium text-yellow-800">No models available</h4>
              <p className="text-sm text-yellow-700">
                Download at least one Whisper model to enable local transcription.
              </p>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}