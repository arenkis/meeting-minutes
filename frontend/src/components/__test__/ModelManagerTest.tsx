import React, { useState } from 'react';
import { ModelManager } from '../ModelManager';
import { ModelDownloadProgress, ProgressRing, DownloadSummary } from '../ModelDownloadProgress';
import { TranscriptSettings, TranscriptModelProps } from '../TranscriptSettings';

// Test component to showcase the new UI components
export function ModelManagerTest() {
  const [selectedModel, setSelectedModel] = useState<string>('large-v3');
  const [transcriptConfig, setTranscriptConfig] = useState<TranscriptModelProps>({
    provider: 'localWhisper',
    model: 'large-v3',
    apiKey: null
  });

  return (
    <div className="max-w-4xl mx-auto p-6 space-y-8">
      <div className="bg-white rounded-lg shadow-lg p-6">
        <h1 className="text-2xl font-bold mb-6 text-gray-900">
          Whisper-rs UI Components Test
        </h1>

        {/* ModelManager Component Test */}
        <section className="mb-8">
          <h2 className="text-xl font-semibold mb-4 text-gray-800">Model Manager</h2>
          <div className="border rounded-lg p-4 bg-gray-50">
            <ModelManager
              selectedModel={selectedModel}
              onModelSelect={(model) => {
                setSelectedModel(model);
                console.log('Selected model:', model);
              }}
            />
          </div>
        </section>

        {/* Progress Components Test */}
        <section className="mb-8">
          <h2 className="text-xl font-semibold mb-4 text-gray-800">Progress Components</h2>
          <div className="space-y-4">
            
            {/* Progress Ring Test */}
            <div className="flex items-center space-x-4 p-4 bg-gray-50 rounded-lg">
              <span className="text-sm font-medium">Progress Ring:</span>
              <ProgressRing progress={25} />
              <ProgressRing progress={50} />
              <ProgressRing progress={75} />
              <ProgressRing progress={100} />
            </div>

            {/* Download Progress Test */}
            <div className="space-y-2">
              <span className="text-sm font-medium">Download Progress:</span>
              <ModelDownloadProgress
                status={{ Downloading: 65 }}
                modelName="medium"
                onCancel={() => console.log('Download cancelled')}
              />
            </div>

            {/* Download Summary Test */}
            <div>
              <span className="text-sm font-medium">Download Summary:</span>
              <DownloadSummary
                totalModels={3}
                downloadedModels={2}
                totalSizeMb={4400}
              />
            </div>
          </div>
        </section>

        {/* Enhanced TranscriptSettings Test */}
        <section className="mb-8">
          <h2 className="text-xl font-semibold mb-4 text-gray-800">Enhanced Transcript Settings</h2>
          <div className="border rounded-lg p-4 bg-gray-50">
            <TranscriptSettings
              transcriptModelConfig={transcriptConfig}
              setTranscriptModelConfig={setTranscriptConfig}
              onSave={(config) => {
                console.log('Saved config:', config);
                alert('Configuration saved! Check console for details.');
              }}
            />
          </div>
        </section>

        {/* State Display */}
        <section className="mb-8">
          <h2 className="text-xl font-semibold mb-4 text-gray-800">Current State</h2>
          <div className="bg-gray-100 p-4 rounded-lg">
            <div className="text-sm font-mono">
              <div><strong>Selected Model:</strong> {selectedModel}</div>
              <div><strong>Transcript Config:</strong></div>
              <pre className="mt-2 text-xs">
                {JSON.stringify(transcriptConfig, null, 2)}
              </pre>
            </div>
          </div>
        </section>

        {/* Feature Showcase */}
        <section className="mb-8">
          <h2 className="text-xl font-semibold mb-4 text-gray-800">Key Features</h2>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <div className="bg-green-50 border border-green-200 rounded-lg p-4">
              <h3 className="font-medium text-green-900 mb-2">✅ Implemented</h3>
              <ul className="text-sm text-green-700 space-y-1">
                <li>• Visual model cards with status indicators</li>
                <li>• Download progress with animations</li>
                <li>• Model selection with immediate feedback</li>
                <li>• TypeScript interfaces for type safety</li>
                <li>• Responsive design with Tailwind CSS</li>
                <li>• Mock data integration for testing</li>
              </ul>
            </div>
            
            <div className="bg-blue-50 border border-blue-200 rounded-lg p-4">
              <h3 className="font-medium text-blue-900 mb-2">🔄 Next Steps</h3>
              <ul className="text-sm text-blue-700 space-y-1">
                <li>• Connect to Tauri backend commands</li>
                <li>• Implement actual model downloads</li>
                <li>• Add model switching functionality</li>
                <li>• Integrate with existing audio pipeline</li>
                <li>• Add error handling and retry logic</li>
                <li>• Performance optimization</li>
              </ul>
            </div>
          </div>
        </section>
      </div>
    </div>
  );
}

// Quick component to test individual features
export function QuickTest() {
  return (
    <div className="p-4 space-y-4">
      <h2 className="text-lg font-bold">Quick UI Test</h2>
      
      {/* Test model status badges */}
      <div className="flex space-x-4">
        <div className="flex items-center space-x-2">
          <div className="w-2 h-2 bg-green-500 rounded-full"></div>
          <span className="text-xs text-green-700">Available</span>
        </div>
        
        <div className="flex items-center space-x-2">
          <div className="w-2 h-2 bg-gray-400 rounded-full"></div>
          <span className="text-xs text-gray-600">Missing</span>
        </div>
        
        <ProgressRing progress={45} size={20} strokeWidth={2} />
      </div>

      {/* Test model cards */}
      <div className="border rounded-lg p-3 max-w-sm">
        <div className="flex items-center space-x-2 mb-2">
          <span className="text-lg">🔥</span>
          <span className="font-medium">Whisper large-v3</span>
          <span className="bg-blue-600 text-white px-2 py-1 rounded-full text-xs">Active</span>
        </div>
        <div className="text-xs text-gray-600 space-x-2">
          <span>📦 3.0GB</span>
          <span>🎯 High accuracy</span>
          <span>⚡ Slow processing</span>
        </div>
      </div>
    </div>
  );
}