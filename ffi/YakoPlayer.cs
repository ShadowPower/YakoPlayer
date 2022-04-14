using System;
using System.Runtime.InteropServices;

namespace YakoPlayer
{
    internal class YakoPlayerNative
    {
        [DllImport("yako_player")]
        internal static extern YakoPlayerHandle yako_player_new();

        [DllImport("yako_player")]
        internal static extern void yako_player_free(IntPtr player);

        [DllImport("yako_player", CharSet = CharSet.Unicode)]
        internal static extern int yako_player_open(YakoPlayerHandle player, string path);

        [DllImport("yako_player")]
        internal static extern int yako_player_play(YakoPlayerHandle player);

        [DllImport("yako_player")]
        internal static extern int yako_player_pause(YakoPlayerHandle player);

        [DllImport("yako_player")]
        internal static extern int yako_player_stop(YakoPlayerHandle player);

        [DllImport("yako_player")]
        internal static extern int yako_player_seek(YakoPlayerHandle player, Int64 position);

        [DllImport("yako_player")]
        internal static extern uint yako_player_get_bitrate(YakoPlayerHandle player);

        [DllImport("yako_player")]
        internal static extern Int64 yako_player_get_duration(YakoPlayerHandle player);

        [DllImport("yako_player")]
        internal static extern Int64 yako_player_get_current_time(YakoPlayerHandle player);

        [DllImport("yako_player")]
        internal static extern int yako_player_is_playing(YakoPlayerHandle player);

        [DllImport("yako_player")]
        internal static extern float yako_player_get_volume(YakoPlayerHandle player);

        [DllImport("yako_player")]
        internal static extern int yako_player_set_volume(YakoPlayerHandle player, float volume);

        [DllImport("yako_player")]
        internal static extern int yako_player_set_mute(YakoPlayerHandle player, int mute);

        [DllImport("yako_player")]
        internal static extern IntPtr yako_player_get_album_cover(YakoPlayerHandle player);

        [DllImport("yako_player")]
        internal static extern uint yako_player_get_album_cover_size(YakoPlayerHandle player);

        [DllImport("yako_player")]
        internal static extern void clear_last_error();

        [DllImport("yako_player")]
        internal static extern int last_error_length();

        [DllImport("yako_player")]
        internal static extern int last_error_length_utf16();

        [DllImport("yako_player")]
        internal unsafe static extern int error_message_utf8(byte* buffer, int length);

        [DllImport("yako_player")]
        internal unsafe static extern int error_message_utf16(byte* buffer, int length);
    }

    internal class YakoPlayerHandle : SafeHandle
    {
        public YakoPlayerHandle() : base(IntPtr.Zero, true) { }

        public override bool IsInvalid
        {
            get { return false; }
        }

        protected override bool ReleaseHandle()
        {
            YakoPlayerNative.yako_player_free(handle);
            return true;
        }
    }

    public class YakoPlayer : IDisposable
    {
        private YakoPlayerHandle player;

        private void CheckError(int returnValue)
        {
            if (returnValue != 0)
            {
                int length = YakoPlayerNative.last_error_length();
                if (length > 0)
                {
                    byte[] buffer = new byte[length];
                    unsafe
                    {
                        fixed (byte* ptr = buffer)
                        {
                            YakoPlayerNative.error_message_utf8(ptr, (int)length);
                        }
                    }
                    string message = System.Text.Encoding.UTF8.GetString(buffer);
                    throw new Exception(message);
                }
            }
        }

        public YakoPlayer()
        {
            player = YakoPlayerNative.yako_player_new();
        }

        public void Open(string filePath)
        {
            CheckError(YakoPlayerNative.yako_player_open(player, filePath));
        }

        public void Play()
        {
            CheckError(YakoPlayerNative.yako_player_play(player));
        }

        public void Pause()
        {
            CheckError(YakoPlayerNative.yako_player_pause(player));
        }

        public void Stop()
        {
            CheckError(YakoPlayerNative.yako_player_stop(player));
        }

        public void Seek(Int64 position)
        {
            CheckError(YakoPlayerNative.yako_player_seek(player, position)); 
        }

        public uint GetBitrate()
        {
            return YakoPlayerNative.yako_player_get_bitrate(player);
        }

        public Int64 GetDuration()
        {
            return YakoPlayerNative.yako_player_get_duration(player);
        }

        public Int64 GetCurrentTime()
        {
            return YakoPlayerNative.yako_player_get_current_time(player);
        }

        public bool IsPlaying()
        {
            return YakoPlayerNative.yako_player_is_playing(player) == 1;
        }

        public float GetVolume()
        {
            return YakoPlayerNative.yako_player_get_volume(player);
        }

        public void SetVolume(float volume)
        {
            CheckError(YakoPlayerNative.yako_player_set_volume(player, volume));
        }

        public void SetMute(bool mute)
        {
            int mute_int = mute ? 1 : 0;
            CheckError(YakoPlayerNative.yako_player_set_mute(player, mute_int));
        }

        public byte[]? GetAlbumCover()
        {
            uint size = YakoPlayerNative.yako_player_get_album_cover_size(player);
            byte[] result = new byte[size];
            IntPtr address = YakoPlayerNative.yako_player_get_album_cover(player);
            if (address == IntPtr.Zero)
            {
                return null;
            }
            Marshal.Copy(address, result, 0, (int)size);
            return result;
        }

        public void Dispose()
        {
            player.Dispose();
        }
    }
}
