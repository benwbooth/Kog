#include <lib/std.mi>
#include attribs/init_Autoresize.m

#define i_playback 30 
#define i_info 40 


//#define i_param 10 

Function buildSkin();
Function fullScreen(boolean onOff);
Function readFrameHeight();
Function readFrameHeight();
Function saveSkinPos();
Function updateMax();

Global Group g, g_screen, g_info, g_playback, g_sui, g_frameBut, g_frameButFS, cprosui;
Global Container player;
Global Layout shade, normal;
Global Layer l_frame1, l_frame2, l_frame3, l_frame4, l_frame5, l_frame6, l_frame7, l_frame8, l_frame9, l_frame2_center;
Global int i_titlebar, i_y, i_center;//, i_info;
Global Button b_goBig, b_goSmall, b_aot;
Global Boolean fullscreen, doubleClick;


//AeroSnap by pjn123
#define i_aerobox_start 40 
#define i_aerobox_screen_side 10 
Global Container c_aerosnap;
Global Layout l_aerosnap;
Global Boolean clipped;
Global int i_clip_w, i_clip_h, i_snapMode; // i_snapMode: 0=disabled, 1=left, 2=top, 3=right

System.onScriptLoaded() {
	initAttribs_Autoresize();

	player = System.getContainer("main");
	normal = player.getLayout("normal");
	//shade = player.getLayout("shade");
	//normal.setRedrawOnResize(0);
	//normal.snapAdjust(i_param, i_param, i_param, i_param);

	cprosui = normal.findObject("centro.mainframe").getParent();

	g = getScriptGroup();
	g_screen = g.getObject("two.screen");
	g_info = g_screen.getObject("two.info");
	g_playback = g_screen.getObject("two.playback");
	g_sui = g.getObject("cpro.sui");
	g_frameBut = g.getObject("two.frame.buttons");
	g_frameButFS = g.getObject("two.frame.buttons.fs");
	
	b_goBig = g_frameBut.getObject("two.frame.goBig");
	b_goSmall = g_frameButFS.getObject("two.frame.goSmall");
	b_aot = g_frameBut.getObject("player.aot");
	
	l_frame1 = g.getObject("two.frame.1");
	l_frame2 = g.getObject("two.frame.2");
	l_frame3 = g.getObject("two.frame.3");
	l_frame4 = g.getObject("two.frame.4");
	l_frame5 = g.getObject("two.frame.5");
	l_frame6 = g.getObject("two.frame.6");
	l_frame7 = g.getObject("two.frame.7");
	l_frame8 = g.getObject("two.frame.8");
	l_frame9 = g.getObject("two.frame.9");
	l_frame2_center = g.getObject("two.frame.2.center");
	
	//read screen height
	/*Map m = new Map;
	m.loadMap("info.bg.seeker.0");
	i_info = m.getHeight();
	delete m;*/

	readFrameHeight();
	updateMax();
	
	
	//g.setXmlParam("minimum_h",integerToString(i_titlebar+i_info+i_playback+8));
	buildSkin();

	fullscreen = getPublicInt("cPro2.fs",false);
	readFrameHeight();
	
	
	//Remember last clip size
	i_clip_w = getPublicInt("cPro2.i_clip_w", 400);
	i_clip_h = getPublicInt("cPro2.i_clip_h", 400);
	//i_snapMode = getPublicInt("cPro2.i_snapMode", 0);
	clipped = getPublicInt("cPro2.clipped", 0);

}
System.onScriptUnLoading(){
	if(normal.isVisible()) saveSkinPos();

}


cprosui.onAction (String action, String param, int x, int y, int p1, int p2, GuiObject source){
	if (strlower(action) == "toggle_fs"){
		if(b_goBig.isVisible()) b_goBig.leftClick();
		else b_goSmall.leftClick();
	}
}

/*
Events order:
	::Going to NORMAL mode
	shade is open
	goto normal
	show normal [save position for SHADE here]
	hide shade [goto saved position for NORMAL here]

	::Going to SHADE mode
	normal is open
	goto shade
	show shade [save position for NORMAL here]
	hide normal [goto saved position for SHADE here]
*/

System.onShowLayout(Layout _layout){
	if(shade == NULL) shade = player.getLayout("shade");

	if(_layout==shade && normal.isVisible()){ 
		saveSkinPos();
	}
	else if(_layout==normal && !shade.isVisible()){
		fullScreen(fullscreen); //On cold start
	}
}
System.onHideLayout(Layout _layout){
	if(_layout==shade && normal.isVisible()){
		fullScreen(fullscreen);
	}
}


normal.onScale (Double newscalevalue){
	if (normal != player.getCurLayout()) return;
	//normal.setXmlParam("maximum_w", integerToString(getViewPortWidthfromGuiObject(normal)/newscalevalue));
	//normal.setXmlParam("maximum_h", integerToString(getViewPortHeightfromGuiObject(normal)/newscalevalue));
	
	updateMax();

	if(getPublicInt("cPro2.fs", 0)==1){
		double newscalevalue = normal.getScale();
		normal.resize(getViewPortLeftfromGuiObject(normal), getViewPortTopfromGuiObject(normal), getViewPortWidthfromGuiObject(normal)/newscalevalue, getViewPortHeightfromGuiObject(normal)/newscalevalue);
	}
}
updateMax(){
	double newscalevalue = normal.getScale();
	normal.setXmlParam("maximum_w", integerToString(getViewPortWidthfromGuiObject(normal)/newscalevalue));
	normal.setXmlParam("maximum_h", integerToString(getViewPortHeightfromGuiObject(normal)/newscalevalue));
}


g.onResize(int x, int y, int w, int h){
	if(i_center<10 || w < i_center+278) l_frame2_center.hide();
	else{
		l_frame2_center.show();
		l_frame2_center.setXmlParam("x", integerToString(w/2-i_center/2));
	}
	
	if(w<280) b_aot.hide();
	else b_aot.show();
}

buildSkin(){
	String temp = integerToString(i_titlebar+i_info+i_playback);
	l_frame4.setXmlParam("y", temp);
	l_frame6.setXmlParam("y", temp);
	
	temp = integerToString(-i_titlebar-i_info-i_playback-8);

	temp = integerToString(i_titlebar);
	l_frame4.setXmlParam("y", temp);
	//l_frame5.setXmlParam("y", temp);
	l_frame6.setXmlParam("y", temp);
	//g_screen.setXmlParam("y", integerToString(i_titlebar));

	temp = integerToString(-i_titlebar-8);
	l_frame4.setXmlParam("h", temp);
	//l_frame5.setXmlParam("h", temp);
	l_frame6.setXmlParam("h", temp);

	g_playback.setXmlParam("y", integerToString(i_info));
	g_info.setXmlParam("h", integerToString(i_info));
	g_screen.setXmlParam("h", integerToString(i_info + i_playback));
	
	//if(l_frame2_center.getAutoWidth()) 

}

saveSkinPos(){
	if(!fullscreen){
		i_y = normal.getTop();
		if(i_y<0) i_y=0;
		setPublicInt("cPro2.x", normal.getLeft());
		setPublicInt("cPro2.y", i_y);
		setPublicInt("cPro2.w", normal.getWidth());
		setPublicInt("cPro2.h", normal.getHeight());
		
		setPublicInt("cPro2.saveby", 0); //0=normal ; 1=shade
	}
}

fullScreen(boolean onOff){
	//normal.hide();
	if(onOff){
		g_frameBut.hide();
		g_screen.setXmlParam("x", "-8");
		g_screen.setXmlParam("w", "16");
		g_screen.setXmlParam("y", integerToString(i_titlebar));
		//g_screen.resize(0,55,0,i_info + i_playback);
		
		l_frame1.setXmlParam("image", "frame.topleft.fs");
		l_frame2.setXmlParam("image", "frame.top.fs");
		l_frame2.setXmlParam("x", "138");
		l_frame2.setXmlParam("w", "-276");
		l_frame2_center.setXmlParam("image", "frame.top.center.fs");
		l_frame3.setXmlParam("image", "frame.topright.fs");
		l_frame3.setXmlParam("x", "-138");
		l_frame4.hide();
		l_frame5.setXmlParam("x", "0");
		l_frame5.setXmlParam("y", integerToString(i_titlebar));
		l_frame5.setXmlParam("w", "0");
		l_frame5.setXmlParam("h", integerToString(-i_titlebar));
		//l_frame5.resize(0,i_titlebar,0,-i_titlebar);
		l_frame6.hide();
		l_frame7.hide();
		l_frame8.hide();
		l_frame9.hide();
		g_frameButFS.show();


		double newscalevalue = normal.getScale();
		normal.resize(getViewPortLeftfromGuiObject(normal), getViewPortTopfromGuiObject(normal), getViewPortWidthfromGuiObject(normal)/newscalevalue, getViewPortHeightfromGuiObject(normal)/newscalevalue);
		//normal.setXmlParam("move", "0");
		
		//Do this last because it takes resources.. must sometime try to clean sui area resize resource hogs
		//g_sui.resize(g_sui.getGuiX(),g_sui.getGuiY()+i_titlebar+i_info+i_playback,0,-(i_titlebar+i_info+i_playback));
		
		g_sui.setTargetX(0);
		g_sui.setTargetY(i_titlebar+i_info+i_playback);
		g_sui.setTargetW(0);
		g_sui.setTargetH(-(i_titlebar+i_info+i_playback));
		g_sui.setTargetSpeed(0);
		g_sui.gotoTarget();

		//g_sui.setXmlParam("x", "0");
		//g_sui.setXmlParam("y", integerToString(i_titlebar+i_info+i_playback));
		//g_sui.setXmlParam("w", "0");
		//g_sui.setXmlParam("h", integerToString(-(i_titlebar+i_info+i_playback)));
		//g_sui.resize(0,i_titlebar+i_info+i_playback,0,-(i_titlebar+i_info+i_playback));
	}
	else{
		g.setXmlParam("minimum_h",integerToString(i_titlebar+i_info+i_playback+8));

		g_frameButFS.hide();
		g_screen.setXmlParam("x", "0");
		g_screen.setXmlParam("w", "0");
		g_screen.setXmlParam("y", integerToString(i_titlebar));
		//g_screen.resize(8,55,-16,i_info + i_playback);

		l_frame1.setXmlParam("image", "frame.topleft");
		l_frame2.setXmlParam("image", "frame.top");
		l_frame2.setXmlParam("x", integerToString(104));
		l_frame2.setXmlParam("w", integerToString(-239));
		l_frame2_center.setXmlParam("image", "frame.top.center");
		l_frame3.setXmlParam("image", "frame.topright");
		l_frame3.setXmlParam("x", integerToString(-135));
		l_frame4.show();
		l_frame5.setXmlParam("x", integerToString(8));
		l_frame5.setXmlParam("y", integerToString(i_titlebar));
		l_frame5.setXmlParam("w", integerToString(-16));
		l_frame5.setXmlParam("h", integerToString(-i_titlebar-8));
		//l_frame5.resize(8,i_titlebar,(-16),(-i_titlebar-8));
		l_frame6.show();
		l_frame7.show();
		l_frame8.show();
		l_frame9.show();
		g_frameBut.show();



		int x = getPublicInt("cPro2.x", getCurAppLeft());
		int y = getPublicInt("cPro2.y", getCurAppTop());
		int w = getPublicInt("cPro2.w", 800);
		int h = getPublicInt("cPro2.h", 600);
		
		if(getPublicInt("cPro2.saveby", 0)==1 && collapse_bottom_attrib.getData() == "1" && linkPosWidth.getData() == "1"){
			y-=h-22;
		}


		normal.resize (x, y, w, h);
		//normal.setXmlParam("move", "1");
	
		//Do this last because it takes resources.. must sometime try to clean sui area resize resource hogs
		//g_sui.resize(g_sui.getGuiX()+8, g_sui.getGuiY()+i_titlebar+i_info+i_playback, -16, -(i_titlebar+i_info+i_playback+8));

		g_sui.setTargetX(8);
		g_sui.setTargetY(i_titlebar+i_info+i_playback);
		g_sui.setTargetW(-16);
		g_sui.setTargetH(-(i_titlebar+i_info+i_playback+8));
		g_sui.setTargetSpeed(0);
		g_sui.gotoTarget();


		//g_sui.setXmlParam("x", "8");
		//g_sui.setXmlParam("y", integerToString(i_titlebar+i_info+i_playback));
		//g_sui.setXmlParam("w", "-16");
		//g_sui.setXmlParam("h", integerToString(-(i_titlebar+i_info+i_playback+8)));
		//g_sui.resize(8,i_titlebar+i_info+i_playback,-16,-(i_titlebar+i_info+i_playback+8));
	}
	//normal.show();
}

readFrameHeight(){
	Map m = new Map;
	if(fullscreen){
		m.loadMap("frame.top.fs");
	}
	else{
		m.loadMap("frame.top");
	}
	i_titlebar = m.getHeight();
	
	if(fullscreen){
		m.loadMap("frame.top.center.fs");
	}
	else{
		m.loadMap("frame.top.center");
	}
	i_center = m.getWidth();
	
	delete m;
}

b_goBig.onLeftClick(){
	saveSkinPos();

	fullscreen=true;
	setPublicInt("cPro2.fs", fullscreen);

	readFrameHeight();
	fullScreen(fullscreen);
}
b_goSmall.onLeftClick(){
	fullscreen=false;
	setPublicInt("cPro2.fs", fullscreen);
	
	readFrameHeight();
	fullScreen(fullscreen);
}



l_frame2.onLeftButtonUp(int x, int y){
	if(doubleClick){
		if(titlebar_dblclk_max_attib.getData() == "1"){
			if(b_goBig.isVisible()){
				b_goBig.leftClick();
			}
			else{
				b_goSmall.leftClick();
			}
		}
		else{
			player.switchToLayout("shade");
		}
	}
	doubleClick=false;
}

l_frame2.onLeftButtonDblClk(int x, int y){
	doubleClick=true;
}











/*
WIN 7 CLIP
*/
normal.onMove(){
	if(aerosnap_attrib.getData() == "0") return;
	if(clipped && normal.getTop()!= getViewPortTopfromGuiObject(normal) ){ //0){
		/*
		Ideally would like to move the layout's x/y position here to make sure mouse stays on window.
		But dont see a function in maki that will enable this :(
		*/
		
		clipped=false;
		setPublicInt("cPro2.clipped", clipped);

		//double toets = System.getMousePosX()-i_clip_w*(System.getMousePosX()/(System.getViewportWidth()/2));
		normal.resize(normal.getLeft(),normal.getTop(),i_clip_w,i_clip_h);
	}
	else if(fullscreen && normal.getLeft() + normal.getTop()!= getViewPortTopfromGuiObject(normal) ){ //0){
		setPublicInt("cPro2.x", normal.getLeft());
		setPublicInt("cPro2.y", normal.getTop());

		b_goSmall.leftClick();
		//return;
	}


	//LEFT SNAP
	if(System.getMousePosX()<1 && i_snapMode==0 ){
		i_snapMode=1;
		c_aerosnap = newDynamicContainer("main.aerosnap");
		l_aerosnap = c_aerosnap.getLayout("normal");

		/*
		See right snap explanation. Too match the right side this animation is also disabled.
		
		l_aerosnap.resize(i_aerobox_screen_side,System.getMousePosY()-i_aerobox_start/2,i_aerobox_start,i_aerobox_start);
		l_aerosnap.setTargetX(i_aerobox_screen_side);
		l_aerosnap.setTargetY(i_aerobox_screen_side);
		l_aerosnap.setTargetW(System.getViewportWidth()/2-i_aerobox_screen_side);
		l_aerosnap.setTargetH(System.getViewportHeight()-i_aerobox_screen_side*2);
		l_aerosnap.setTargetSpeed(0.3);
		l_aerosnap.gotoTarget();
		*/
		
		l_aerosnap.resize(i_aerobox_screen_side,i_aerobox_screen_side,System.getViewportWidth()/2-i_aerobox_screen_side,System.getViewportHeight()-i_aerobox_screen_side*2);

		l_aerosnap.show();
	}
	else if(System.getMousePosX()>15 && i_snapMode==1){
		i_snapMode = 0;
		c_aerosnap.close();
		c_aerosnap = NULL;
		l_aerosnap = NULL;
	}


	//RIGHT SNAP
	if(System.getMousePosX()== getViewportWidth()-1 && i_snapMode==0){
		i_snapMode=3;
		c_aerosnap = newDynamicContainer("main.aerosnap");
		l_aerosnap = c_aerosnap.getLayout("normal");

		/*
		Winamp can't resize the frame so that it looks like the frame is anchored to the right of the screen.
		Doesn't look smooth, so disabling the animation :(
		Might save a few cycles though :)
		
		l_aerosnap.resize(System.getMousePosX()-i_aerobox_start-i_aerobox_screen_side,System.getMousePosY()-i_aerobox_start/2,i_aerobox_start,i_aerobox_start);
		l_aerosnap.setTargetX(System.getViewportWidth()/2);
		l_aerosnap.setTargetY(i_aerobox_screen_side);
		l_aerosnap.setTargetW(System.getViewportWidth()/2-i_aerobox_screen_side);
		l_aerosnap.setTargetH(System.getViewportHeight()-i_aerobox_screen_side*2);
		l_aerosnap.setTargetSpeed(0.3);
		l_aerosnap.gotoTarget();*/
		l_aerosnap.resize(System.getViewportWidth()/2,i_aerobox_screen_side,System.getViewportWidth()/2-i_aerobox_screen_side,System.getViewportHeight()-i_aerobox_screen_side*2);

		l_aerosnap.show();
	}
	else if(System.getMousePosX()< System.getViewportWidth()-15 && i_snapMode==3){
		i_snapMode = 0;
		c_aerosnap.close();
		c_aerosnap = NULL;
		l_aerosnap = NULL;
	}


	//Top SNAP
	if(System.getMousePosY()<1 && i_snapMode==0 && !fullscreen){
		i_snapMode=2;
		c_aerosnap = newDynamicContainer("main.aerosnap");
		l_aerosnap = c_aerosnap.getLayout("normal");

		/*
		See right snap explanation. Too match the right side this animation is also disabled.
		
		l_aerosnap.resize(i_aerobox_screen_side,System.getMousePosY()-i_aerobox_start/2,i_aerobox_start,i_aerobox_start);
		l_aerosnap.setTargetX(i_aerobox_screen_side);
		l_aerosnap.setTargetY(i_aerobox_screen_side);
		l_aerosnap.setTargetW(System.getViewportWidth()/2-i_aerobox_screen_side);
		l_aerosnap.setTargetH(System.getViewportHeight()-i_aerobox_screen_side*2);
		l_aerosnap.setTargetSpeed(0.3);
		l_aerosnap.gotoTarget();
		*/
		
		l_aerosnap.resize(i_aerobox_screen_side,i_aerobox_screen_side,System.getViewportWidth()-i_aerobox_screen_side*2,System.getViewportHeight()-i_aerobox_screen_side*2);

		l_aerosnap.show();
	}
	else if(System.getMousePosY()>15 && i_snapMode==2){
		i_snapMode = 0;
		c_aerosnap.close();
		c_aerosnap = NULL;
		l_aerosnap = NULL;
	}
}

normal.onEndMove(){
	if(aerosnap_attrib.getData() == "0") return;
	//debug("123");
	if(i_snapMode==0) return;
	
	i_clip_w = normal.getWidth();
	i_clip_h = normal.getHeight();

	if(i_snapMode==1){
		//Delete frame
		i_snapMode = 0;
		c_aerosnap.close();
		c_aerosnap = NULL;
		l_aerosnap = NULL;

		//Resize player
		clipped=true;
		normal.resize(0,0,System.getViewportWidth()/2,System.getViewportHeight());
	}
	else if(i_snapMode==2){
		//Delete frame
		i_snapMode = 0;
		c_aerosnap.close();
		c_aerosnap = NULL;
		l_aerosnap = NULL;

		b_goBig.leftClick();
	}
	else if(i_snapMode==3){
		//Delete frame
		i_snapMode = 0;
		c_aerosnap.close();
		c_aerosnap = NULL;
		l_aerosnap = NULL;

		//Resize player
		clipped=true;
		normal.resize(System.getViewportWidth()/2,0,System.getViewportWidth()/2,System.getViewportHeight());
	}
	
	setPublicInt("cPro2.i_clip_w",i_clip_w);
	setPublicInt("cPro2.i_clip_h",i_clip_h);
	setPublicInt("cPro2.clipped", clipped);
}