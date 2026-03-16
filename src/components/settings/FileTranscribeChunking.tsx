import React from "react";
import { useTranslation } from "react-i18next";
import { useSettings } from "../../hooks/useSettings";
import { Input } from "../ui/Input";
import { SettingContainer } from "../ui/SettingContainer";

interface FileTranscribeChunkingProps {
  descriptionMode?: "tooltip" | "inline";
  grouped?: boolean;
}

export const FileTranscribeChunking: React.FC<FileTranscribeChunkingProps> = ({
  descriptionMode = "inline",
  grouped = false,
}) => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating } = useSettings();

  const chunkingEnabled = getSetting("file_transcribe_chunking") ?? true;
  const chunkSeconds =
    (getSetting("file_transcribe_chunk_seconds") as number) ?? 120;

  const handleToggle = () => {
    updateSetting("file_transcribe_chunking", !chunkingEnabled);
  };

  const handleSecondsChange = (event: React.ChangeEvent<HTMLInputElement>) => {
    const value = parseInt(event.target.value, 10);
    if (!isNaN(value) && value >= 10) {
      updateSetting("file_transcribe_chunk_seconds", value);
    }
  };

  return (
    <SettingContainer
      title={t("settings.debug.fileTranscribeChunking.title")}
      description={t("settings.debug.fileTranscribeChunking.description")}
      descriptionMode={descriptionMode}
      grouped={grouped}
      layout="horizontal"
    >
      <div className="flex items-center gap-2">
        <button
          onClick={handleToggle}
          disabled={isUpdating("file_transcribe_chunking")}
          className={`relative inline-flex h-5 w-9 items-center rounded-full transition-colors ${
            chunkingEnabled ? "bg-accent" : "bg-mid-gray/30"
          }`}
        >
          <span
            className={`inline-block h-3.5 w-3.5 transform rounded-full bg-white transition-transform ${
              chunkingEnabled ? "translate-x-4" : "translate-x-0.5"
            }`}
          />
        </button>
        {chunkingEnabled && (
          <>
            <Input
              type="number"
              min="10"
              max="600"
              value={chunkSeconds}
              onChange={handleSecondsChange}
              disabled={isUpdating("file_transcribe_chunk_seconds")}
              className="w-20"
            />
            <span className="text-sm text-text">
              {t("settings.debug.fileTranscribeChunking.seconds")}
            </span>
          </>
        )}
      </div>
    </SettingContainer>
  );
};
