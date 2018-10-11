pub const POST_VERTEX_SHADER: &str = r"
#version 150 core

in vec2 a_Pos;
in vec2 a_TexCoord;
out vec2 v_TexCoord;

void main() {
	v_TexCoord = a_TexCoord;
	gl_Position = vec4(a_Pos, 0.0, 1.0);
}
";

pub const POST_PIXEL_SHADER: &str = r"
#version 150 core

uniform sampler2D t_Source;

in vec2 v_TexCoord;
out vec4 o_Color;

void main() {
	o_Color = texture(t_Source, v_TexCoord, 0);
}
";

pub const POST_PIXEL_SHADER_MSAA_4X: &str = r"
#version 150 core

uniform sampler2DMS t_Source;

in vec2 v_TexCoord;
out vec4 o_Color;

void main() {
	vec2 d = textureSize(t_Source);
	ivec2 i = ivec2(d * v_TexCoord);
	o_Color = (texelFetch(t_Source, i, 0) + texelFetch(t_Source, i, 1)
			+ texelFetch(t_Source, i, 2) + texelFetch(t_Source, i, 3)) / 4.0;
}
";
