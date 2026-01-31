defmodule PhotoFinish.Events.FolderGenerator do
  @moduledoc """
  Generates folder structure on disk for an event.
  Creates Gym and Session folders based on event configuration.
  """

  @doc """
  Creates folder structure for an event.

  Given storage_root="/mnt/nas/event", num_gyms=2, sessions_per_gym=3:
  Creates:
    /mnt/nas/event/Gym A/Session 1A
    /mnt/nas/event/Gym A/Session 2A
    /mnt/nas/event/Gym A/Session 3A
    /mnt/nas/event/Gym B/Session 1B
    /mnt/nas/event/Gym B/Session 2B
    /mnt/nas/event/Gym B/Session 3B

  Returns {:ok, paths} where paths is a list of created directories,
  or {:error, reason} if folder creation fails.
  """
  @spec create_event_folders(String.t(), pos_integer(), pos_integer()) ::
          {:ok, [String.t()]} | {:error, term()}
  def create_event_folders(storage_root, num_gyms, sessions_per_gym) do
    paths =
      for gym_num <- 1..num_gyms,
          session_num <- 1..sessions_per_gym do
        letter = gym_letter(gym_num)
        gym_folder = "Gym #{letter}"
        session_folder = "Session #{session_num}#{letter}"
        Path.join([storage_root, gym_folder, session_folder])
      end

    try do
      Enum.each(paths, &File.mkdir_p!/1)
      {:ok, paths}
    rescue
      e in File.Error ->
        {:error, e.reason}
    end
  end

  @doc """
  Converts a gym number (1, 2, 3...) to a letter (A, B, C...)
  """
  @spec gym_letter(1..26) :: String.t()
  def gym_letter(gym_num) when gym_num >= 1 and gym_num <= 26 do
    <<?A + gym_num - 1>>
  end
end
