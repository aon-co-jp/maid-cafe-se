package tokyo.runo.maidcafese.core.audio

/** TTS音声の元の性別(端末の音声名からの推定)。声の加工の元になる値で、加工そのものはRust側。 */
enum class SourceGender { FEMALE, MALE, UNKNOWN }
