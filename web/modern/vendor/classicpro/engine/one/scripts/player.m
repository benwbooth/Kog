#include <lib/std.mi>
#include <lib/config.mi>
#include <lib/pldir.mi>
#include attribs/init_Autoresize.m
#include <lib/quickPlaylist.mi>
#include <lib/fileio.mi>
#include <lib/application.mi>
#include "../../lib/ClassicProFile.mi"

//#define rres 20 

// Function updateTextSizes();
Function saveResize(int x, int y, int w, int h);
Function gotoGlobal();
Function saveGlobal();
Function updateMax();
Function String getMyFile();

// Mute warning
Function doFade();
Function stopFade();
Function startFade();
Global Boolean direction;
Global Timer myTimer;


Global Group mainGroup, menuBar;
Global Container main;
Global Layout mylayout;
Global Slider vol_slider, seek_slider;
Global Layer vol_layer, tt_layer, bgLeft, bgRight, bgTop, bgBottom, bgLeftRead, bgTopRead, vol_layer2, muteWarning;
Global GuiObject progressbar, seekBg;
Global Vis mainVis;
Global Text tracktimer, infodisplay;
Global GuiObject trackTitle, vol_images;
Global Button bolt, fakeAbout, changeTheme, sysMenu, sysMenuFake;
Global Popupmenu selMenu;
Global XmlDoc myDoc;

//MuteButton
Global Togglebutton mute_but;
Global Timer reCheck;

//Global Layer resize1, resize2, resize3, resize4, resize6, resize7, resize8, resize9;
Global Layer resize6, resize8, resize9;
Global Boolean mouseDown, dontResize, checkHeightAgain, docked;
Global int i, lastKnownW, lastKnownH, rres, i_minw;

System.onScriptLoaded() {
	initAttribs_Autoresize();

	rres=20;

	mainGroup = getScriptGroup();
	vol_slider = mainGroup.findObject("volume");
	seek_slider = mainGroup.findObject("seeker");
	vol_layer = mainGroup.findObject("volume.bg");
	vol_images = mainGroup.findObject("volume.images");
	vol_layer2 = mainGroup.findObject("volume.bg2");
	vol_layer.setXmlParam("w", integerToString(97/255*getVolume()));
	progressbar = mainGroup.findObject("progressbar");
	seekBg = mainGroup.findObject("progressbar.bg");
	mute_but = mainGroup.findObject("mute");
	muteWarning = mainGroup.findObject("mute.warning");
	mainVis = mainGroup.findObject("main.vis");
	tt_layer = mainGroup.findObject("hidden.tracktime");
	tracktimer = mainGroup.findObject("SongTime");
	trackTitle = mainGroup.findObject("infosongticker");
	infodisplay = mainGroup.findObject("fileinfo.mode.2.text");
	bolt = mainGroup.findObject("winampbolt");
	fakeAbout = mainGroup.findObject("aboutwinamp.hidden");
	changeTheme = mainGroup.findObject("Cpro.theme.next");
	sysMenu = mainGroup.findObject("player.sysmenu");
	sysMenuFake = mainGroup.findObject("player.sysmenu.fake");
	menuBar = mainGroup.findObject("player.mainmenu");

	bgLeftRead = mainGroup.findObject("read.bg.left");
	bgTopRead = mainGroup.findObject("read.bg.top");

	bgLeft = mainGroup.findObject("cpro.bg.left");
	bgRight = mainGroup.findObject("cpro.bg.right");
	bgTop = mainGroup.findObject("cpro.bg.top");
	bgBottom = mainGroup.findObject("cpro.bg.bottom");
	
	main = System.getContainer("main");
	mylayout = main.getLayout("normal");
	updateMax();
	
	reCheck = new Timer;
	reCheck.setDelay(10);
	
	myTimer = new Timer;
	myTimer.setDelay(700);
	
	i_minw = stringToInteger(mainGroup.getXmlParam("minimum_w"));

	
	//resize1 = mainGroup.findObject("resizer.1");
	//resize2 = mainGroup.findObject("resizer.2");
	//resize3 = mainGroup.findObject("resizer.3");
	//resize4 = mainGroup.findObject("resizer.4");
	resize6 = mainGroup.findObject("resizer.6");
	//resize7 = mainGroup.findObject("resizer.7");
	resize8 = mainGroup.findObject("resizer.8");
	resize9 = mainGroup.findObject("resizer.9");

	if(getStatus()==STATUS_STOPPED){
		progressbar.hide();
		seekBg.hide();
	}
	
	if(getPrivateInt(getSkinName(), "muted", 0)==1){
		mute_but.setActivated(true);
		mute_but.setXmlParam("tooltip", "Turn Volume On");
		startFade();
	}
	
		
	// Reader for classic vis colors from bitmap (wa5.51)
	Map myMap = new Map;
	myMap.loadMap("vis.color.read");
	
	mainVis.setXmlParam("colorbandpeak",integerToString(myMap.getARGBValue(0,0,2))+","+integerToString(myMap.getARGBValue(0,0,1))+","+integerToString(myMap.getARGBValue(0,0,0)));
	
	for(i=2;i<18;i++){
		mainVis.setXmlParam("colorband"+integerToString(18-i),integerToString(myMap.getARGBValue(0,i,2))+","+integerToString(myMap.getARGBValue(0,i,1))+","+integerToString(myMap.getARGBValue(0,i,0)));
	}

	for(i=0;i<5;i++){
		mainVis.setXmlParam("colorosc"+integerToString(5-i),integerToString(myMap.getARGBValue(2,i,2))+","+integerToString(myMap.getARGBValue(2,i,1))+","+integerToString(myMap.getARGBValue(2,i,0)));
	}

	//Support for colorthemes needs a bolt that can be semi transparent [cpro v1.03]
	myMap.loadMap("buttons.png");
	if(myMap.getWidth()==332){
		bolt.setXmlParam("image", "winamp.logo.1");
		bolt.setXmlParam("downImage", "winamp.logo.3");
	}
	delete myMap;
	
	
	if(vol_layer2.isInvalid()){
		vol_layer.show();
	}
	else{
		vol_images.show();
	}


	if(tt_layer.isInvalid()){
		tracktimer.setXmlParam("font", "cpro.tracktime.font");
	}

	if(!bgLeftRead.isInvalid()){
		bgLeft.setXmlParam("image", "player.left.alt");
		bgLeft.setXmlParam("tile", "0");
		bgRight.setXmlParam("image", "player.right.alt");
		bgRight.setXmlParam("tile", "0");
	}
	if(!bgTopRead.isInvalid()){
		bgTop.setXmlParam("image", "player.top.alt");
		bgTop.setXmlParam("tile", "0");
		bgBottom.setXmlParam("image", "player.bottom.alt");
		bgBottom.setXmlParam("tile", "0");
	}
	
	if(getPrivateInt(getSkinName(), "muted", 0)==1){
		mute_but.setActivated(true);
		mute_but.setXmlParam("tooltip", "Turn Volume On");
	}
	else{
		mute_but.setActivated(false);
		mute_but.setXmlParam("tooltip", "Mute Volume");
	}

	myDoc = new XmlDoc;
	String fullpath = getParam()+"ClassicPro.xml";
	myDoc.load(fullpath);
	
	if(myDoc.exists()){
		myDoc.parser_addCallback("ClassicPro/TextSettings*");
		myDoc.parser_start();
		myDoc.parser_destroy();
	}
	delete myDoc;
}
System.onScriptUnloading(){
	if(getPublicInt("cPro.lastmode", 0)==0) saveGlobal(); //0=normal ; 1=shade
	
	setPrivateInt(getSkinName(), "muted", mute_but.getCurCfgVal());
	setPublicInt("cPro.firstlayout", 0);
	delete reCheck;
	delete myTimer;
}

mainGroup.onSetVisible(boolean onOff){
	if(onOff)	i_minw = stringToInteger(mainGroup.getXmlParam("minimum_w"));

}

myDoc.parser_onCallback (String xmlpath, String xmltag, list paramname, list paramvalue){
	if(strlower(xmltag) == "style"){
		String busyWith ="";
		for(int i=0; i<paramname.getNumItems(); i++){
			if(strlower(paramname.enumItem(i))=="id"){
				busyWith=paramvalue.enumItem(i);
			}
			
			if(busyWith=="normal.songticker"){
				trackTitle.setXmlParam(paramname.enumItem(i),paramvalue.enumItem(i));
			}
			else if(busyWith=="normal.tracktime"){
				tracktimer.setXmlParam(paramname.enumItem(i),paramvalue.enumItem(i));
			}
			else if(busyWith=="normal.trackinfo"){
				infodisplay.setXmlParam(paramname.enumItem(i),paramvalue.enumItem(i));
			}
		}
	}
}

myLayout.onResize(int x, int y, int w, int h){
	lastKnownW = w;
	lastKnownH = h;
}

gotoGlobal(){
	if(docked) return;

	int x = getPublicInt("cPro.x", getCurAppLeft());
	int y = getPublicInt("cPro.y", getCurAppTop());
	int w = getPublicInt("cPro.w", getCurAppWidth());
	int h = getPublicInt("cPro.h", getCurAppHeight());

	if(getPublicInt("cPro.maximized", 0)==1){
		double newscalevalue = mylayout.getScale();
		x=getViewPortLeftfromGuiObject(mylayout);
		y=getViewPortTopfromGuiObject(mylayout);
		w=getViewPortWidthfromGuiObject(mylayout)/newscalevalue;
		h=getViewPortHeightfromGuiObject(mylayout)/newscalevalue;
	}

	
	if(w<i_minw) w= i_minw;
	if(h<168) h= 168; //why was this w=168??? just change back if any bugs arise
	
	//debug(integerToString(x)+","+integerToString(y)+","+integerToString(w)+","+integerToString(h));

	if(getPublicInt("cPro.saveby", 0)==1 && collapse_bottom_attrib.getData() == "1"){
		if(getPublicInt("cPro.maximized", 0)==1) y = getPublicInt("cPro.y", getCurAppTop());
		y-=h-23;
		//debug(integerToString(x)+","+integerToString(y)+","+integerToString(w)+","+integerToString(h));
	}

	//Winshade -> Normal : Bottom of screen
	if(y>=System.getViewportHeight()-23) y=System.getViewportHeight()-h;

	//removed x < 0 as multiple monitors can contain negative x,y values

	//debugstring("resize to: "+integerToString(x)+", "+integerToString(y)+", "+integerToString(w)+", "+integerToString(h), 9);
	if(checkHeightAgain) mylayout.resize(mylayout.getLeft(),mylayout.getTop(),lastKnownW,h);
	else mylayout.resize(x,y,w,h);
}

saveGlobal(){
	if(main.getCurLayout() == mylayout){
		setPublicInt("cPro.x", mainGroup.getLeft());
		setPublicInt("cPro.y", mainGroup.getTop());
		setPublicInt("cPro.w", lastKnownW);
	}
	setPublicInt("cPro.h", lastKnownH);
	setPublicInt("cPro.saveby", 0); //0=normal ; 1=shade

}

/* dont think i need this... removed for 1.04!
System.onCreateLayout(Layout _layout){
	if(_layout==mylayout){
		gotoGlobal();
		//debugstring("a  == "+integerToString(getPublicInt("cPro.w", getCurAppWidth())), 9);
	}
}*/

System.onShowLayout(Layout _layout){
	if(dontResize){
		if(getPrivateInt(getSkinName(), "muted", 0)==1){
			mute_but.setActivated(true);
			mute_but.setXmlParam("tooltip", "Turn Volume On");
		}
		else{
			mute_but.setActivated(false);
			mute_but.setXmlParam("tooltip", "Mute Volume");
		}

	}

	/*if(_layout==mylayout && !dontResize && getPublicInt("cPro.firstlayout", 0)==0){
		setPublicInt("cPro.firstlayout", 1);
		reCheck.start();
		dontResize=true;
	}
	else if(_layout==mylayout && !dontResize && !checkHeightAgain ){
		checkHeightAgain=true;
		reCheck.start();
		dontResize=true;
	}
	else */
	if(_layout==mylayout){
		setPublicInt("cPro.lastmode", 0); //0=normal ; 1=shade
		reCheck.start();
	}
}
System.onHideLayout(Layout _layout){
	if(_layout==mylayout){
			saveGlobal();
	}
}


reCheck.onTimer(){
	reCheck.stop();
	gotoGlobal();
}

/*tracktimer.onTextChanged(String newtxt){
	updateTextSizes();
}*/
/*updateTextSizes(){
	debugstring("mohaha"+integerToString(tracktimer.getAutoWidth()), 9);
	trackTitle.setXmlParam("x", integerToString(tracktimer.getTextWidth()+15));
	trackTitle.setXmlParam("w", "-"+integerToString(tracktimer.getTextWidth()+15+14));
	infodisplay.setXmlParam("x", integerToString(tracktimer.getTextWidth()+15));
	infodisplay.setXmlParam("w", "-"+integerToString(tracktimer.getTextWidth()+15+14));
}*/

vol_slider.onSetPosition(int newpos){
	vol_layer.setXmlParam("w", integerToString(97/255*newpos));
	
	if(mute_but.getCurCfgVal()==1 && newpos>0){
		mute_but.setActivated(false);
		setPrivateInt(getSkinName(), "muted", 0);
		stopFade();
	}
}
vol_slider.onPostedPosition(int newpos){
	vol_layer.setXmlParam("w", integerToString(97/255*newpos));
	if(mute_but.getCurCfgVal()==1 && newpos>0){
		mute_but.setActivated(false);
		setPrivateInt(getSkinName(), "muted", 0);
		stopFade();
	}
}

System.onStop(){
	progressbar.hide();
	seekBg.hide();
}
System.onPlay(){
	progressbar.show();
	seekBg.show();
}
System.onResume(){
	progressbar.show();
	seekBg.show();
}

mute_but.onToggle(Boolean onoff){
	if(mute_but.getCurCfgVal()==0){
		setVolume(getPrivateInt(getSkinName(), "saveVol", 100));
		mute_but.setXmlParam("tooltip", "Mute Volume");
		stopFade();
	}
	else{
		setPrivateInt(getSkinName(), "saveVol", getVolume());
		setVolume(0);
		mute_but.setXmlParam("tooltip", "Turn Volume On");
		startFade();
	}
	setPrivateInt(getSkinName(), "muted", mute_but.getCurCfgVal());
}


seek_slider.onEnterArea(){
	progressbar.setXmlParam("left", "seeker.bg.left.3");
	progressbar.setXmlParam("middle", "seeker.bg.center.3");
	progressbar.setXmlParam("right", "seeker.bg.right.3");
}
seek_slider.onLeaveArea(){
	progressbar.setXmlParam("left", "seeker.bg.left.2");
	progressbar.setXmlParam("middle", "seeker.bg.center.2");
	progressbar.setXmlParam("right", "seeker.bg.right.2");
}


vol_slider.onEnterArea(){
	vol_layer.setXmlParam("image", "volume.bg.hover");
}
vol_slider.onLeaveArea(){
	vol_layer.setXmlParam("image", "volume.bg");
}

bolt.onLeftClick(){
	int a = getPublicInt("cPro.multibutton", 0);
	
	if(a==0){
		fakeAbout.setXmlParam("action", "TOGGLE");
		fakeAbout.setXmlParam("param", "guid:{D6201408-476A-4308-BF1B-7BACA1124B12}");
		fakeAbout.leftClick();
	}
	else if(a==1){
		//System.navigateUrl(getPath(getPlayItemMetaDataString("filename")));
		ClassicProFile.exploreFile(getMyFile());
	}
	else if(a==2){
		popQuickPlaylist(System.getViewportHeight()/24, false); //was 40
	}
	else if(a==3){
		changeTheme.leftClick();
	}
	else if(a==4){
		fakeAbout.setXmlParam("param", "");
		fakeAbout.setXmlParam("action", "ML_SendTo");
		fakeAbout.leftClick();
	}
	else if(a==5){
		fakeAbout.setXmlParam("param", "");
		fakeAbout.setXmlParam("action", "trackmenu");
		fakeAbout.leftClick();
	}
	
}
String getMyFile() {
	String bs = strleft("\ ",1);
	String output = "";

	if(System.strleft(System.getPlayItemString(),6) == "cda://") output = System.strmid(System.getPlayItemString(), 6, 1)+":"+bs;
	else output= getPlayItemMetaDataString("filename");
	return output;
}

bolt.onRightButtonUp(int x, int y){
	selMenu = new PopupMenu;
	selMenu.addCommand("Multi-Button Action:", -1, 0, 1);
	selMenu.addSeparator();
	selMenu.addCommand("Change Color Theme", 3, 0, 0);
	selMenu.addCommand("Explore Folder", 1, 0, 0);
	selMenu.addCommand("Open About Winamp", 0, 0, 0);
	selMenu.addCommand("Show Quick Playlist", 2, 0, 0);
	selMenu.addCommand("Show Send to Menu", 4, 0, 0);
	selMenu.addCommand("Show Track Menu", 5, 0, 0);
	selMenu.checkCommand(getPublicInt("cPro.multibutton", 0), 1);
	
	//int a = selMenu.popAtMouse();
	int a = selMenu.popAtXY(clientToScreenX(bolt.getLeft())+2, clientToScreenY(bolt.getTop())+26);
	// clientToScreenX(trigger.getLeft()), clientToScreenY(trigger.getTop()+26)
	
	if(a>=0){
		setPublicInt("cPro.multibutton", a);
	}
	Complete;
	delete selMenu;
}sysMenu.onLeftButtonDblClk(int x, int y){
	Application.Shutdown();
}
sysMenu.onLeftClick(){
	if(System.isKeyDown(VK_CONTROL)){
		selMenu = new PopupMenu;
		selMenu.addCommand("Show menu bar", 1, getPublicInt("cPro.menubar", 1), 0);
	
		int a = selMenu.popAtMouse();
		
		if(a>=0){
			setPublicInt("cPro.menubar", !getPublicInt("cPro.menubar", 1));
		}
		menuBar.sendAction("update_menu", "", 0, 0, 0, 02);
		
		Complete;
		delete selMenu;
	}
	else{
		sysMenuFake.leftClick();
		complete;
	}
}
sysMenu.onRightButtonUp(int x, int y){
	ClassicProFile.exploreFile(getMyFile());
	complete;
}


/*	Classic Resizers ;)
*/
resize6.onLeftButtonDown(int x, int y){mouseDown=true;}
resize6.onLeftButtonUp(int x, int y){mouseDown=false;}
resize6.onMouseMove(int x, int y){
	if(mouseDown){
		x=x-(x-i_minw)%rres;
		saveResize(myLayout.getLeft(), myLayout.getTop(),x+10,myLayout.getHeight());
	}
}

resize8.onLeftButtonDown(int x, int y){mouseDown=true;}
resize8.onLeftButtonUp(int x, int y){mouseDown=false;}
resize8.onMouseMove(int x, int y){
	if(mouseDown){
		y=y-(y-168)%rres;
		saveResize(myLayout.getLeft(), myLayout.getTop(), myLayout.getWidth(),y+10);
	}
}
resize9.onLeftButtonDown(int x, int y){mouseDown=true;}
resize9.onLeftButtonUp(int x, int y){mouseDown=false;}
resize9.onMouseMove(int x, int y){
	if(mouseDown){
		x=x-(x-i_minw)%rres;
		y=y-(y-168)%rres;
		saveResize(myLayout.getLeft(), myLayout.getTop(),x+10,y+10);
	}
}


saveResize(int x, int y, int w, int h){
	if(docked) return;
	if(getPublicInt("cPro.maximized", 0)==1) return;
	
	if(w<i_minw) w=i_minw;
	if(h<220) h=168;
	
	updateMax();
	
	myLayout.resize(x,y,w,h);
}


mylayout.onDock(int side){
	docked=true;
}

mylayout.onUndock(){
	docked=false;
}

updateMax(){
	double newscalevalue = mylayout.getScale();
	mylayout.setXmlParam("maximum_w", integerToString(getViewPortWidthfromGuiObject(mylayout)/newscalevalue));
	mylayout.setXmlParam("maximum_h", integerToString(getViewPortHeightfromGuiObject(mylayout)/newscalevalue));
}


//Mute warning
myTimer.onTimer(){
	doFade();
}
doFade(){
	if(direction){
		muteWarning.setTargetA(0);
	}
	else{
		muteWarning.setTargetA(253);
	}
	muteWarning.setTargetSpeed(0.6);
	muteWarning.gotoTarget();
	direction=!direction;
}
startFade(){
	direction=false;
	doFade();
	myTimer.start();
	muteWarning.show();
}
stopFade(){
	myTimer.stop();
	muteWarning.cancelTarget();
	muteWarning.setAlpha(0);
	muteWarning.hide();
}