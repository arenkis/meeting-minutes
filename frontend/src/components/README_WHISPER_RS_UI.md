# Whisper-rs UI Components Documentation

This document describes the new UI components created for the whisper-rs migration, which enhance model management and user experience.

## Components Overview

### 1. ModelManager (`./ModelManager.tsx`)

The main component for managing Whisper models with visual cards and status indicators.

**Features:**
- Visual model cards with performance indicators
- Real-time download progress
- Model selection with immediate feedback
- Automatic model discovery
- Status badges (Available, Missing, Downloading, Error)

**Props:**
```typescript
interface ModelManagerProps {
  selectedModel?: string;
  onModelSelect?: (modelName: string) => void;
  className?: string;
}
```

**Usage:**
```tsx
<ModelManager
  selectedModel="large-v3"
  onModelSelect={(model) => console.log('Selected:', model)}
/>
```

### 2. ModelDownloadProgress (`./ModelDownloadProgress.tsx`)

Progress indicators for model downloads with animations and status feedback.

**Components included:**
- `ModelDownloadProgress`: Full progress bar with cancel functionality
- `ProgressRing`: Circular progress indicator
- `DownloadSummary`: Overview of available models and storage

**Usage:**
```tsx
<ModelDownloadProgress
  status={{ Downloading: 65 }}
  modelName="medium"
  onCancel={() => console.log('Cancelled')}
/>

<ProgressRing progress={75} size={40} strokeWidth={3} />

<DownloadSummary
  totalModels={3}
  downloadedModels={2}
  totalSizeMb={4400}
/>
```

### 3. Enhanced TranscriptSettings (`./TranscriptSettings.tsx`)

Updated transcript settings component that integrates the new ModelManager.

**New Features:**
- Integrated ModelManager for Local Whisper
- Visual provider selection with icons
- Contextual help information
- Removed API key requirement for Local Whisper
- Smart model selection

## Type Definitions

### Core Types (`../types/whisper.ts`)

```typescript
interface ModelInfo {
  name: string;
  path: string;
  sizeMb: number;
  accuracy: ModelAccuracy;
  speed: ProcessingSpeed;
  status: ModelStatus;
  description?: string;
}

type ModelAccuracy = 'High' | 'Good' | 'Decent';
type ProcessingSpeed = 'Slow' | 'Medium' | 'Fast';
type ModelStatus = 
  | 'Available' 
  | 'Missing' 
  | { Downloading: number } 
  | { Error: string };
```

### Model Configurations

Predefined configurations for each model:

```typescript
export const MODEL_CONFIGS = {
  'large-v3': {
    description: 'Highest accuracy, best for important meetings. Slower processing.',
    sizeMb: 3000,
    accuracy: 'High',
    speed: 'Slow'
  },
  'medium': {
    description: 'Balanced accuracy and speed. Good for most use cases.',
    sizeMb: 1400,
    accuracy: 'Good',
    speed: 'Medium'
  },
  'small': {
    description: 'Fast processing with good quality. Great for quick transcription.',
    sizeMb: 465,
    accuracy: 'Decent',
    speed: 'Fast'
  }
};
```

## Visual Design System

### Color Scheme
- **Available**: Green (`bg-green-100 text-green-800`)
- **Missing**: Gray (`bg-gray-100 text-gray-800`)
- **Downloading**: Blue (`bg-blue-100 text-blue-800`)
- **Error**: Red (`bg-red-100 text-red-800`)

### Icons and Emojis
- 🔥 High accuracy models
- ⚡ Balanced models
- 🚀 Fast models
- 📦 File size
- 🎯 Accuracy indicator
- ⚡ Speed indicator
- 🏠 Local processing
- ☁️ Cloud services

### Status Indicators
- ✓ Available (green dot)
- 📥 Download required (gray dot)
- ProgressRing for downloading
- ❌ Error state (red dot)

## Testing

### Test Component (`__test__/ModelManagerTest.tsx`)

A comprehensive test component showcasing all new UI features:

- Model Manager with mock data
- Progress components with different states
- Enhanced TranscriptSettings integration
- State debugging and feature showcase

**To use the test component:**
1. Import and add to your route/page
2. Check console for interaction logs
3. Test different model selections
4. Verify progress animations

### Mock Data

The components include mock data for testing:
- 3 models (large-v3, medium, small)
- All models show as "Available" (existing models)
- Realistic file sizes and performance characteristics
- Simulated download progress (200ms intervals)

## Integration Guide

### Step 1: Import Components
```tsx
import { ModelManager } from './components/ModelManager';
import { TranscriptSettings } from './components/TranscriptSettings';
```

### Step 2: Add to Settings Page
```tsx
// For standalone model management
<ModelManager
  selectedModel={currentModel}
  onModelSelect={handleModelChange}
/>

// For integrated transcript settings
<TranscriptSettings
  transcriptModelConfig={config}
  setTranscriptModelConfig={setConfig}
  onSave={handleSave}
/>
```

### Step 3: Handle State Updates
```tsx
const [selectedModel, setSelectedModel] = useState('large-v3');
const [config, setConfig] = useState({
  provider: 'localWhisper',
  model: 'large-v3',
  apiKey: null
});

const handleModelSelect = (modelName: string) => {
  setSelectedModel(modelName);
  // Update backend model selection here
};
```

## Backend Integration Points

### Tauri Commands (To Be Implemented)

The UI components expect these Tauri commands:

```rust
// Get available models
#[tauri::command]
async fn get_available_models() -> Result<Vec<ModelInfo>, String>

// Download a model
#[tauri::command]
async fn download_model(model_name: String) -> Result<(), String>

// Switch active model
#[tauri::command]
async fn switch_whisper_model(model_name: String) -> Result<(), String>

// Get download progress (via events)
#[tauri::command]
async fn get_download_progress(model_name: String) -> Result<f32, String>
```

### Event Listeners

For real-time progress updates:

```typescript
import { listen } from '@tauri-apps/api/event';

// Listen for download progress
listen('model-download-progress', (event) => {
  const { modelName, progress } = event.payload;
  updateModelStatus(modelName, { Downloading: progress });
});

// Listen for download completion
listen('model-download-complete', (event) => {
  const { modelName } = event.payload;
  updateModelStatus(modelName, 'Available');
});
```

## Responsive Design

All components are built with responsive design:
- Mobile-first approach
- Flexible grid layouts
- Scalable text and icons
- Touch-friendly interactive elements

### Breakpoints
- Small screens: Single column layouts
- Medium screens: Two-column grids
- Large screens: Full feature display

## Accessibility Features

- Semantic HTML structure
- ARIA labels for interactive elements
- Keyboard navigation support
- Color contrast compliance
- Screen reader friendly text
- Focus indicators

## Performance Considerations

- Lazy loading for model information
- Optimized re-renders with React.memo
- Efficient state management
- Progressive enhancement
- Minimal bundle impact

## Future Enhancements

### Planned Features
1. **Model Recommendations**: AI-powered model selection based on system specs
2. **Usage Analytics**: Track model performance and usage patterns
3. **Custom Models**: Support for user-provided Whisper models
4. **Batch Operations**: Download multiple models simultaneously
5. **Storage Management**: Disk space monitoring and cleanup tools
6. **Performance Metrics**: Real-time transcription speed and accuracy stats

### Extensibility
The component architecture supports:
- Plugin system for additional model types
- Theme customization
- Custom progress indicators
- Additional cloud providers
- Advanced configuration options

## Troubleshooting

### Common Issues

1. **Models not showing**: Check file permissions and path configuration
2. **Download failures**: Verify internet connection and disk space
3. **Selection not working**: Ensure onModelSelect callback is provided
4. **Styling issues**: Verify Tailwind CSS classes are available

### Debug Mode

Enable debug logging:
```typescript
const DEBUG = true;
console.log('Model state:', { selectedModel, availableModels });
```

### Performance Debugging

Monitor render performance:
```typescript
import { Profiler } from 'react';

<Profiler id="ModelManager" onRender={onRenderCallback}>
  <ModelManager />
</Profiler>
```